[Heap Allocation](https://os.phil-opp.com/heap-allocation/)

# Heap Allocation のメモ

heap allocation をサポートする. 

---
TODO: 今までは heap を使えなかったから static を使っていたという認識でいい? static には gloabl variable としての役割がある. IDT 等のカーネル内で使われ続けるものは static でいい. ほかの static もそうといえるか? WRITER は?

-> 後で local/static 変数の役割, 利点・欠点が説明されている. 

---

## Local and Static Variables

現在 ローカル変数と `static` 変数の 2種類の変数をカーネルで使っている. 
ローカル変数は call statck 上に保存され, 関数が return する間のみ有効. 
static 変数は fixed memory location に保存され, プログラムの lifetime 全体で生存する. 

### Local Variables

ローカル変数は call stack に保存されるが, call stack は `push` 及び `pop` 命令をサポートするスタックデータ構造である. 
各関数に入るときに, パラメータやリターンアドレス, 呼び出された関数のローカル変数がコンパイラによって push される. 

```rust
// -------------
// x = 1
// -------------
// y 
// =============
// i = 1
// -------------
// return address
// -------------
// z[0] = 1
// ...


fn outer() {
    let x = 1;
    let y = inner(x);
}

fn inner(i: usize) -> u32 {
    let z = [1,2,3];
    z[i]
}

```

まず `outer` のローカル変数が call stack に含まれていることがわかる. 
`outer` 関数が `inner` 関数を呼び出すとき, パラメータ `1` とリターンアドレスが push される. 
そして制御が `inner` へと渡され, `inner` はローカル変数を push する. 

`inner` 関数が return すると, `inner` の call stack は pop され, `outer` のローカル変数のみが残る. 


`inner` のローカル変数が関数リターンまでしか生存しないことを理解した. 
Rust のコンパイラはこのライフタイムを強制し, 違反した場合はエラーを投げる. 

エラーが出る例: ローカル変数の参照を返す
```rust
fn inner(i: usize) -> &'static u32 {
    let z = [1, 2, 3];
    &z[i] // `inner` が return した後は生存していない. -> エラー
}
```

### Static Variables
static 変数は, スタックとは別の, メモリの固定位置に保存される. 
このメモリの場所はコンパイル時にリンカによって指定される. 
static はプログラムのランタイムと同じだけ生存し, そのため `'static` lifetime を持ち, ローカル変数から参照可能となる. 

例: 
```rust
fn outer() {
    let x = 1;
    let y = inner(x);
}

static Z: [u32; 3] = [1,2,3];

fn inner(i: usize) -> &'static u32 { // 変数の lifetime と返り値の lifetime が同一
    &Z[i] // 参照先の `Z` は runtime の lifetime を持つので OK
}
```

`'static` lifetime とは別に, static 変数にはその場所がコンパイル時に知ることができるという便利な性質がある, なので static 変数にアクセスするのに参照は必要としない. 
この特徴は `println` マクロに使われており, 内部で static な `Writer` を使うことで, `&mut Writer` 参照を必要としないマクロ呼び出しが可能となった. このことは他の変数にアクセスできない例外ハンドラに便利である. 

TODO: どういうこと? -> 例外ハンドラは他の関数から参照を持ってくることができない. しかし static ならいつでもアクセス可能.

しかし, static 変数の性質には重大な欠点がある. 
それは通常では読み取り専用 read-only なこと. 
2つのスレッドが同時に static 変数を変更するなどしてデータ競合が発生する可能性があるため, Rust は read-only を強制している. 
static 変数を変更する唯一の方法は `Mutex` 型でカプセル化 encapsulate することで, これによって複数の `&mut` 参照が同時に存在しないことを保証できる. 

TODO: `Mutex` 型であれば static は変更可能というのは Rust で保証されていること? `lazy_static` の使用には `Mutex` が必要?

### Dynamic Memory

ローカル変数, static 変数はどちらも強力だが, 制約もある: 
- ローカル変数は関数かスコープの終わりまでしか生存しない. これは関数 return 時に call stack が破壊されるから. 
- static 変数はプログラムのランタイム全体で生存するため, 変数が不必要になったとしても変数の再宣言やメモリの再利用ができない. また, static 変数は所有権のセマンティクスが曖昧で, どの関数からでもアクセス可能なため, 変更のためには `Mutex` で保護される必要がある. 

もう一つのローカル/static 変数への制約は, 固定サイズしか保有できないこと. 
なのでローカル/static 変数では vector や string などを保存できない. 

この欠点を回避するため, プログラミング言語は heap と呼ばれる変数を保存する第3のメモリ領域をサポートすることが多い. 
heap は `allocate`/`deallocate` 関数を通して, ランタイム時の dynamic memory allocation をサポートしている. 

これは次のように動作する: `allocate` 関数が変数保存に使うことができる指定されたサイズの空のメモリチャンクを返す. この変数は `deallocate` 関数が変数への参照を引数として呼ばれることで free されるまで生存する. 

例: 
```rust
// 実行後のヒープ
// ----------------
// z[0] = 1
// ----------------
// 
// ----------------
// z[2] = 3
// ----------------

fn outer() {
    let x = 1;
    let y = inner(x);
    deallocate(y, size_of(u32));
}

fn inner(i: usize) -> *mut u32 {
    let z = allocate(size_of([u32; 3]));
    z.write([1,2,3]);
    (z as *mut u32).offset(i) // 生ポインタに, offset を追加
}
```

簡単にメモリリークが発生する. 

### Common Errors

メモリリークはプログラムの脆弱性にはならないが, それよりも深刻なバグとして以下の2つが挙げられる: 
- `deallocate` を呼び出したあとにその変数を使い続けてしまうとき: use-after-free. このバグは未定義動作を引き起こし, 攻撃者に任意コード実行されかねない. 
- 変数を二度 free してしまうとき: double-free. これは同じ場所に存在する別のアロケーションを free する可能性があり, その場合は use-after-free を引き起こしかねない. 

これらの脆弱性は広く知られているが, それにもかかわらず未だによく見つかる脆弱性となっている. 
Linux にも use-after-free から任意コード実行する脆弱性があったりする. 

これらの問題を解決するため, Java や Python などの多くの言語が garbage collection を使って動的メモリを自動で管理している. 
この考えではプログラマは `deallocate` 関数を手動で呼び出すことはせず, その変わりに変数は自動で deallocate される. 
そのため, 上記の脆弱性は発生しない. 
欠点は定期的なスキャンによる性能のオーバーヘッドで, 停止時間が長くなることもある. 

Rust では別の手法を採用しており, 動的メモリの操作をコンパイル時に検証する **所有権** という考えを使っている. 
そのため上記の脆弱性を回避するために GC は必要でなく, したがってパフォーマンスオーバーヘッドもない. 
所有権システムのもう一つの利点はプログラマが動的メモリの使用をコントロール可能なこと. 

### Allocation in Rust

プログラマに `allocate`/`deallocate` を手動で呼び出させる代わりに, Rust の標準ライブラリにはこれらの関数を暗黙に実行する抽象的な型を提供している. 
最も重要なのは `Box` で, これは heap-allocated value の抽象である. 
`Box::new` コンストラクタで値を取り, 内部で `allocate` を呼んで heap の allocated slot に値を move する. 
これを free するため `Box` 型は `Drop` トレイトを実装しており, これによってスコープを抜けると `deallocate` される. 

このパターンは resource acquisition is initialization (RAII) と呼ばれている. 

---
TODO: RAII とはなに?
> RAII（Resource Acquisition Is Initialization）は、日本語では「リソース取得は初期化である」「リソースの確保は初期化時に」「リソースの取得と初期化」などの意味を持ち、資源（リソース）の確保と解放を、クラス型の変数の初期化と破棄処理に結び付けるというプログラミングのテクニックである。特にC++とD言語で一般的であり、デストラクタをサポートしないC言語などに対する優位性や利便性のうちのひとつとなっている。

参考: [RAII - Wikipedia](https://ja.wikipedia.org/wiki/RAII) 

---

この型単独ではすべての use-after-free を防ぐことができない. 

例:
```rust
let x = {
    let z = Box::new([1,2,3]);
    &z[1] // z[i] はすぐ deallocate される
}; 
println!("{}", x); // use-after-free
```

しかし, ここで Rust の所有権を使うと, それぞれの参照に lifetime をつけ, 参照のスコープを決定することができる. 

上の例の場合, 参照 `x` は `z` から来ているが, `z` がスコープ外になったあとは無効になる. 

---
TODO: `Box` って参照外ししなくても要素アクセスできる?

```rust
let x = Box::new([1, 2, 3]);
println!("{}", x[0]) // OK
println!("{}", (*x)[0]) // OK
```

-> できた

rust には implicit deref coercion 暗黙的な参照外し型強制がある. たぶんそれが動いている?

-> 違うかも
```rust
// A stack-allocated array
let array: [i32; 3] = [1, 2, 3]; // 定数でサイズを指定

// A heap-allocated array, coerced to a slice
let boxed_array: Box<[i32]> = Box::new([1, 2, 3]); // サイズを指定する必要なし
```
[Array types - The Rust Reference](https://doc.rust-lang.org/stable/reference/types/array.html) より

ヒープ上の配列はスライスと同じ性質を持つよう強制される (?)  
-> スライスと同じ操作が可能.

---

### Use Cases

dynamic memory allocation 動的メモリ確保 はいつ使うべき? 

メモリ確保時にヒープ領域から空いている場所を探す必要があるため, 動的メモリ確保にはパフォーマンスオーバーヘッドがかかる. 
このため性能を重視するカーネルコードではローカル変数が好まれる. 
しかし, 動的メモリ確保が最良の選択であるケースもある.


