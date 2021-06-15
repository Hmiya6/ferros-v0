URL: [A Freestanding Rust Binary](https://os.phil-opp.com/freestanding-rust-binary/)
# A Freestanding Rust Binary の一次メモ

OS のカーネルを書くには、OS 機能に依存しないコードが必要. 

ほとんどの標準ライブラリは使用不可能. 使える機能も多い.  
E.g. iterators, closures, pattern matching, option and result, string formatting, and ownership system

-> Rust で特徴的なメモリ安全性や未定義動作の心配をすることなくカーネルを書ける


## 標準ライブラリの無効化

OS や, `libc` に依存している機能は使えない

`no_std` : 自動で標準ライブラリが含まれる機能を無効化

`no_std` について: [A `no_std` Rust Environment](https://docs.rust-embedded.org/book/intro/no-std.html)


## `no_std`

```rust
#![no_std]

fn main() {
    println!("Hello, world!"); // error
}
```

`println` マクロが使用できない (標準ライブラリのマクロであるため).

さらに `println` を消しても, 
```
error: language item required, but not found: `eh_personality`

error: `#[panic_handler]` function required, but not found
```
というエラーがでる.

## `panic_handler`

`panic` が起きたときに呼ばれる関数を指定する必要がある

```rust
// `core` は `no_std` でも使える
use core::panic::PanicInfo;

#[panic_handler] // panic したときに呼ばれる関数を指定
fn panic(_info: &PanicInfo) -> ! { // diverging function と呼ばれる、return しない関数
    loop {}
}

```

`-> !` に関しては [Diverging functions](https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions) を参照

`PanicInfo` はどのファイルのどの行でパニックが起きたかの情報を保持する

特にできることもなく、`return` もできないので `loop`.

## `eh_personality`

`eh_personality` は `lang_items` の一つ.  
`lang_items` については [lang_items](https://doc.rust-lang.org/unstable-book/language-features/lang-items.html). 

`lang_items` について
> The `rustc` compiler has certain pluggable operations, that is, functionality that isn't hard-coded into the language, but is implemented in libraries, with a special marker to tell the compiler it exists. The marker is the attribute `#[lang = "..."]` and there are various different values of `...`, i.e. various 'lang items'.

訳
> `rustc` コンパイラにはいくつかの pluggable な操作があり, つまりその機能は言語にはハードコードされておらずライブラリに実装されているようなものがあり, その機能が存在していることを特殊なマーカーでコンパイラに伝える必要がある. そのマーカーが `#[lang = "..."]` 属性であり, 様々な種類がある.

中でも `eh_personality` は stack unwinding を実装するためのもの.

call stack は実行中の関数に関する情報を保存するスタック. call stack が存在するものな理由は, active functions のそれぞれが実行完了時にどのポイントにコントロールを戻すかを追跡するため. (ここでの active functions は, 呼ばれたものの return によって実行が完了していいない関数のこと.) stack unwinding は関数のスタックを解放する (変数の場合はデコンストラクタが呼ばれる).  

参考:
- [Stack Unwinding](https://www.bogotobogo.com/cplusplus/stackunwinding.php)  
- [コールスタック](https://ja.wikipedia.org/wiki/%E3%82%B3%E3%83%BC%E3%83%AB%E3%82%B9%E3%82%BF%E3%83%83%E3%82%AF)

通常では, Rust は panic 時にすべての生存しているスタック変数のデコンストラクタを走らせるために unwinding を使う. これによって使用しているメモリをすべて解放して親スレッドが panic を捕えて実行をつづけることができる. が, unwinding は複雑で OS 依存のライブラリを必要とする.

---
疑問: 関数の `return` と unwinding は何が違うのか.  

-> `return` はプログラムが行うもの. 逆に unwinding はホスト側が (panic をキャッチして) 行うもの. (?)  
-> unwinding はプログラムがクラッシュしても動き続けるプログラムとして実装する必要がある (?)

---

## Unwinding の無効化
unwinding の実装 (= `eh_personality` の実装) はまだ難しいので, `panic` 時に `abort` (中断) が起こるように設定する. 

`Cargo.toml` に追加:
```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```
これで `cargo build` すると, 
```
error: requires `start` lang_item

error: aborting due to previous error

error: could not compile `ferros`

To learn more, run the command again with --verbose.
```
`start` が足りないためにエラーになっている.

## `start` 属性

ほとんどの言語にはランタイムシステムがある, GC やスレッドを管理する. このラインラムは`main` 関数が呼ばれる前に初期化を行うために呼ばれる.

一般的な Rust バイナリ (stdlib をリンクするもの) では, 実行は `crt0` (C runtime zero) と呼ばれる C ランタイムライブラリで始まる. `crt0` は C アプリのために環境を立ち上げる. これにはスタックの確保や引数を正しいレジスタに配置することが含まれる. C ランタイムは次に Rust ランタイムの開始ポイントを呼び出す. この 'Rust ランタイムの開始ポイント' は `start` language item として印をつけられている. Rust にはごく最小限のランタイムしかなく, スタックオーバーフロー防御の立ち上げや panic 時における backtrace の表示などの小さいことのみを扱う. そのあとランタイムはようやく `main` 関数を呼ぶ.

---
更に調べること: rust runtime と c runtime の違い


---

Rust コンパイラに通常の開始ポイント連鎖を使わないことを伝えるため, `#![no_main]` 属性を追加する


OS による開始ポイントを上書きする
```rust
#[no_mangle] // `_start` をそのまま出力したい
// `extern "C"` で C の呼出規約を用いいて関数を呼び出す
pub extern "C" fn _start() -> ! {
    loop{}
}
```
`#[no_mangle]` 属性を使うことで, name mangling を無効化できる. これにより Rust コンパイラは `_start`という名前で関数を出力する (これを行わない場合, コンパイラは関数名を一意にするため関数名に余計な文字列を付加する). `_start` と明示することで, リンカに開始ポイントの関数名を伝えることができるようになる.

`extern "C"` をつけることで C の呼出規約を用いる. 関数名 `_start` はほとんどのシステムで標準的な開始ポイント名となる.

---
補足:

> We also have to mark the function as `extern "C"` to tell the compiler that it should use the C calling convention for this function (instead of the unspecified Rust calling convention). The reason for naming the function `_start` is that this is the default entry point name for most systems.

> `extern "C"` をつけることで C の呼出規約を用いる. 関数名 `_start` はほとんどのシステムで標準的な開始ポイント名となる.

`extern "C"` で C の呼び出し規約を用いると, どんなメリットがあるのか.  
-> どんなシステム (= CPU) でも追加の設定なしで使える.

他の呼び出し規約 (例えば Rust) だと, CPU が対応していない(可能性がある). (?)

このチュートリアルでは system (システム) が CPU の意味で使われることがある. OS を指していることもある.
- Almost all x86 systems have ...
- ... we want to compile for a clearly defined target system.
- Cargo supports different target systems through the `--target` parameter.

---

`!` で diverging function となる (diverging fucntion = return できない関数). 開始ポイントは関数から呼ばれるものでなく (よって return すべき呼出元が存在しない), システムから直接呼ばれるので必要. return する代わりに開始ポイントは, 例えば OS の `exit` システムコールなどを呼び出すなどしなければならない. 今回ではマシンをシャットダウンするのがよい.

```
error: linking with `cc` failed: exit code: 1
  |
  = note: "cc" "-Wl,--as-needed" "-Wl,-z,noexecstack" "-m64" "-Wl,--eh-frame-hdr" "-L" ...
```
リンクエラー.. (次回で解決)





### 補足: 呼出規約について

From [呼出規約 - Wikipedia](https://ja.wikipedia.org/wiki/%E5%91%BC%E5%87%BA%E8%A6%8F%E7%B4%84)

> ABI の一部で, サブルーチン (引用者注: 関数など) が呼び出されるときに従わなければならない制限などの標準である.

1. 名前修飾 (mangling) 
2. 実引数・リターンアドレス・戻り値のどのように格納するか
3. 各レジスタを呼び出し側とサブルーチン側のどちらで保存するか
etc...

From [呼出規約 - ertl.jp](http://ertl.jp/~takayuki/readings/info/no04.html)

> プログラムが計算を行うためには, レジスタを使用する必要があります. しかし, 呼出元も呼出先もレジスタを使用するので, そのままではレジスタを取り合ってケンカがおきます. これを調停するのが呼出規約です.


例: `stdcall`

呼出元:
```
push arg2
push arg1
call label
mov retval, eax
```

呼出先:
```
label:
    push ebp
    mov ebp, esp
    sub esp, buflength
    mov eax, [ebp+0x8]
    mov ebx, [ebp+0xC]
    (processing)
    mov esp, ebp
    pop ebp
    ret
```

#### calling
1. `arg1`, `arg2` を push したあと
スタックの様子
```
// EIP = `call label`

===== <- EBP
.
.
-----
arg2
-----
arg1 
===== <- ESP

```

2. `call label` のあと
`call` 命令は, `EIP` の値を `push` してから `EIP` を `label` で指定したアドレスに更新する命令.

スタックの様子
```
// EIP = `push ebp`

================ <- EBP
.
.
----------------
arg2
----------------
arg1
----------------
リターンアドレス (呼出元の EIP)
================ <- ESP
```

3. `push ebp` の後
古い `ebp` を退避する

スタックの様子
```
// EIP = `mov ebp, esp`

================ <- EBP
.
.
----------------
arg2
----------------
arg1
----------------
retaddr
----------------
ebp のバックアップ (古い ebp)
================ <- ESP
```

4. `mov ebp, esp` の後
ebp を esp に合わせて新しいスタックを開始

スタックの様子
```
// EIP = `esp, buflength`

================
.
.
----------------
arg2
----------------
arg1
----------------
retaddr
----------------
ebp のバックアップ (古い ebp)
================ <- ESP, EBP
```
#### returning

1. `mov esp, ebp` の後
スタックをクリア

スタックの様子
```
// EIP = `pop ebp`

================
.
.
----------------
arg2
----------------
arg1
----------------
retaddr
----------------
ebp のバックアップ (古い ebp)
================ <- ESP, EBP
```

2. `pop ebp` のあと
ebp を復元

スタックの様子
```
// EIP = `ret`

================ <- EBP
.
.
----------------
arg2
----------------
arg1
----------------
retaddr
================ <- ESP
```

3. `ret` のあと
`ret` 命令はスタックの先頭要素を `EIP` に書く格納して, `ESP` を 1 つ小さくする.

スタックの様子
```
// EIP = `mov retval, eax`

================ <- EBP
.
.
----------------
arg2
----------------
arg1
================ <- ESP
```





























