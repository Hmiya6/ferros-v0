[Async/Await](https://os.phil-opp.com/async-await/)

# Async/Await のメモ

Rust の cooperative multitasking と async/await について考える. 


## Multitasking

OS の根本的な機能のひとつは multitasking で, これによって複数のタスクを並行に実行することが可能になる. 

すべてのタスクが並列に動作しているように見えるが, 実際には一つの CPU コアは同時に一つのプログラムのみしか実行できない. 
並列にタスクを走らせているように見せかけるため, OS は高速に実行中のタスクを切り替えている. 
コンピュータは高速なのでこの切り替え作業に気づくことはない. 

シングルコアの CPU は同時に一つのタスクのみを実行可能だが, マルチコアの CPU は複数のタスクを実際に並列に走らせることができる. 
例えば, 8コアの CPU は同時に 8つのタスクを同時に実行することが可能である. 
ここでは, 簡便のためにシングルコア CPU を取り扱う. 

マルチタスクには 2つの形態がある: 
一つは協調的マルチタスク cooperative multitasking で, タスクが定期的に CPU の制御を明け渡すことで別のタスクを走らせる. 
もう一つは非協調的マルチタスク Preemptive multitasking (preemtive 先取の) で, OS の機能を使ってスレッドを強制的に一時停止させることで任意の時点で切り替える. 

### Preemptive Multitasking
preemtive multitasking は OS がタスクスイッチを制御する. 
ここでは, 割り込み時に OS が CPU 制御を獲得することを使用している. 
例えば, マウスが動いたりネットワークパケットが届いたりするとタスクスイッチが起こりうる. 
OS は一つのタスクが動作可能な時間をハードウェアタイマーで決定し, その時間が経過すると割り込みを発する. 

#### Saving State
タスクは任意のタイミングで割り込まれるため, 計算の途中である可能性もある. 
あとでタスクの再開が可能なように, OS はタスクの状態を call stack や CPU レジスタの値を含めてバックアップしておかなければならない. 
このプロセスを context switch と呼ぶ. 

call stack はとても巨大になりうるため, OS はタスクごとに call stack を分割してセットアップすることが多い (すべてのタスクの call stack が一つになっている場合, call stack 全体をバックアップすることになる). 
別のスタックをもったタスクのことを thread of execution と呼び, 省略して thread と呼ぶ. 
この方法ではコンテクストスイッチのオーバーヘッドを最小化する, この最適化は重要で, それは毎秒 100回コンテクストスイッチが行われる可能性もあるため. 

#### Discussion
preemptive multitasking の主な利点は OS がタスクの実行時間を完全に制御できる点だ. 
この方法では各タスクが協調を行うことなく十分な CPU 時間を確保することが保証される. 
これは特に third-party のタスクや複数のユーザがシステムを共有するような場合に重要となる. 

preemption の欠点は各タスクが各自の stack を保持している必要があること. 
共有の stack と比較すると, タスクごとのメモリ消費量が大きくなりタスクの数が制限されることになる. 
もう一つの欠点は OS がタスクスイッチごとに CPU レジスタの状態を完璧に保存しておく必要があること. 

TODO: タスクごとのメモリ消費量が大きくなりタスクの数が制限される. そんなに深刻なこと? どの程度大きくなる? それとも組み込み機器の話?

信頼できないユーザースペースのプログラムを走らせることができるため, preemptive multitasking とスレッドは OS の根本的な構成部品である. 
これは別の章で扱う. 

## Aync/Await in Rust

Rust は async/await という形で cooperative multitasking を最高の形でサポートしている. 
async/await が何なのか, またどう動くのかを検討する前に, まずは future と非同期プログラミングが Rust でどのように動くのかを理解する必要がある. 

TODO: async/await は cooperative multitasking の別名? async/await はどこから来た概念? python にはあった. go などの他言語には存在している? 

### Futures

futures はまだ利用可能でないかもしれない値を表現している (future は日本語だと未来という意味になりがちだが, 「先物(取引)」という意味もある). 
例えば, 別のタスクによって計算されている integer やネットワークからダウンロード中のファイルがそれにあたりうる. 
futures を使うと, 値が利用可能になるのを待つのではなく, その値が必要になるまで実行を続けることが可能になる. 

#### Example

```
┌────┐   ┌───────────┐   ┌───┐
│main│   │file system│   │foo│
└─┬──┘   └────┬──────┘   └─┬─┘
  │           │            │
  │async_read │            │
  ├──────────►│            │
  │  Future   │            │
  │◄──────────┤            │
  │           │   foo      │
  ├───────────┼───────────►│
  │  resource │            │
  │◄──────────┤            │
  │           │ return     │
  │◄──────────┼────────────┤
  │           │            │
```

非同期な `async_read` では, ファイルシステムは future を返してファイルはバックグラウンドで非同期にロードする. 
これによって `main` 関数は `foo` をずっと早く呼べる. 

#### Futures in Rust

Rust では futures は `Futures` トレイトで表現されている: 
```rust
pub trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>;
}
```

associated type `Output` は非同期の値の型を指定する. 
例えば, `async_read_file` では `Future` のインスタンスが `Output` を `File` として返される. 

`poll` メソッドによって値が利用可能であるかを確認できる. 
`poll` メソッドは `Poll` enum を返す. 

```rust
pub enum Poll<T> {
    Ready(T),
    Pending,
}
```

---
TODO: `Ready(T)` は可能? `struct Foo(usize)` と同じ? 

> 列挙型(enum)はいくつかの異なる型の中から1つを選ぶような場合に使用する。構造体(struct)の定義を満たすものならば何でもenum 内の型として使用できる

[列挙型 - Rust by Example 日本語版](https://doc.rust-jp.rs/rust-by-example-ja/custom_types/enum.html) より

---

値が利用可能であれば (e.g. ファイルがディスクから読み取り完了した), その値は `Ready` 型で wrap されて返される. 
そうでなければ, `Pending` 型が返され, caller に対してまだ値が利用可能でないことをシグナルする. 

`poll` メソッドはふたつの引数をとる: 一つは `self: Pin<&mut Self>` で, もう一つは `cs: &mut Context`. 
前者は通常の `&mut self` 参照と同様に動作するが, `Self` の値がそのメモリ位置に pin されているところが異なる. 
`Pin` と何故それが必要なのかを理解するには async/await がどのように動作するかを先に理解する必要がある. 

`cx: &mut Context` 引数の目的は `Waker` インスタンスを非同期タスクへと渡すこと. 
`Waker` によって, 非同期タスクが終了したことを非同期タスク自身がシグナルできるようになる, e.g. ディスクからのファイルのロードが完了したこと. 
メインタスクは `Future` の準備が完了したときに通知されることを知っているため, `poll` を何度も呼び出す必要がない. 

TODO: `Waker` からのシグナル "`Future` is ready!"-> `poll` で結果を確認する. この流れであってる? > 最初の `poll` で `Waker` を `Context` で wrap して渡し, `Waker` からのシグナルを待つ. 

### Working with Futures

futures がどのように定義されるかを確認し, `poll` メソッドについて基本的な考え方を理解した. 
しかし, futures との上手い付き合い方を知らない. 
問題は futures が非同期タスクの結果を表現しており, しかもその結果はまだ利用可能でないかもしれないことである. 
では, 必要になったときに future の値をとってくるにはどうすればいいか? 

#### Waiting on Futures
これに対する一つの解答案は future の準備が完了するまで待機することである. 

```rust
let future = asnyc_read_file("foo.txt");
let file_content = loop {
    match future.poll(...) {
        Poll::Ready(value) => break value,
        Poll::Pending => {}, // do nothing
    }
}
```

ここでは `poll` を何度もループで呼び出すことで future を積極的に actively 待っている. 
この解決策は動作はするものの, 値が利用可能になるまで CPU が常に busy となるためとても非効率的. 

さらに効率的は手法はスレッドを future が利用可能になるまでブロックしてしまうことだろう. 
この手法はスレッドが存在する場合のみ利用可能なので, このカーネルでは機能しない (少なくとも今は). 
しかも, スレッドのブロックがサポートされているシステムだとしても, スレッドのブロックは非同期タスクを同期タスクに変えてしまうためによくない. 

#### Future Combinators

代替案は future combinator を使用すること. 
future combinator は `map` のようなメソッドで, futures を連鎖させ, 組み合わせることを可能にし, `Iterator` のメソッドに似ている. 
Instead of waiting on the future, these combinators return a future themselves, which applies the mapping operation on `poll`. 

例えば, `string_len` combinator は `Future<Output=String>` を `Future<Output=usize>` へと変換する. 

```rust
// 実際には動かないコード

struct StringLen<F> {
    inner_future: F,
}

impl<F> Future for StringLen<F> 
where F: Future<Output = String> {
    type Output = usize;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        // 内部の future を poll する. -> future を future のまま計算している
        match self.inner_future.poll(cx) { 
            Poll::Ready(s) => Poll::Ready(s.len()),
            Poll::Pending => Poll::Pending,
        }
    }
}

// 
fn string_len(string: impl Future<Output = String>) 
-> impl Future<Output = usize> {
    StringLen {
        inner_future: string,
    }
}

fn file_len() -> impl Future<Output = usize> {
    let file_content_future = async_read_file("foo.txt");
    string_len(file_content_future)
}
```

pin の処理をしていないので実際は動かないコード. 

`string_len` 関数の基本的な考え方は任意の `Future` インスタンスを新しい `StringLen` 構造体で wrap していることで, `StringLen` も `Future` を実装している. 
wrap された future が poll されるとき, `StringLen` の inner future を poll する. 
値が準備できている場合, 文字列が `Poll::Ready` 型から抜き取られ, その長さが計算される. 
その後, `Poll::Ready` に wrap されて返される. 

`string_len` 関数によって非同期な文字列の長さをそれを待つことなく計算する. 
この関数は `Future` を返すので, 呼び出し側は返り値を直接操作することはできず, また combinator 関数を使用する必要がある. 
こうすることで, 呼び出し関係の全体が非同期になり, メイン関数などの特定のポイントで効率的に複数の future を待つことが可能になる. 

自力で combinator 関数を書くのは難しいので, ライブラリで提供されることが多い. 
Rust の標準ライブラリは combinator method を提供していないが, 準公式ライブラリである (そして `no_std` にも対応している) `futures` クレートが提供している. 
`futures` クレートの `FutureExt` トレイトは `map` や `then` などの高レベルの combinator method を提供している. 

##### Advantages

future combinator の大きな利点は操作を非同期に保ち続けることができること. 
非同期 I/O インターフェイスを組み合わせることで, この手法は非常に高いパフォーマンスに達する. 
future combinator が trait implementaion とともに通常の構造体として実装されている事実によって, コンパイラによる過剰な最適化を可能とする. 
さらなる詳細については [Zero-cost futures in Rust](https://aturon.github.io/blog/2016/08/11/futures/) を参照. 

##### Drawbacks

future combinators が効率的なコードを可能にする一方で, その型やクロージャを用いたインターフェイスのために使いにくい状況もある (コードが複雑になる. 可読性が低下する). 

```rust
fn example(min_len: usize) -> impl Future<Output = String> {
    async_read_file("foo.txt").then(move |content| { // content: String
        if content.len() < min_len {
            Either::Left(async_read_file("bar.txt").map(|s| content + &s) /* future::Map 型 */)
        } else {
            Either::Right(future::ready(content) /* future::Ready 型 */))
        }
    })
}
```

`Either` wrapper によって if/else における型の違いを吸収している. 

### The Async/Await Pattern

async/await の背景にはプログラマに通常の同期的なコードと同様にコードを書かせてコンパイラに非同期なコードに変換させたいという考え方がある. 
Rust の `async` キーワードは, 同期的な関数を, future を返す非同期的な関数へと変化させる関数シグネチャである. 

```rust
async fn foo() -> u32 {
    0
}

// 上のコードはコンパイラによって大体以下のように変換される. 
fn foo() -> impl Future<Output = u32> {
    future::ready(0)
}
```

このキーワードだけではあまり使えない. 
しかし, `async` 関数内では, future の非同期の値をとってくるための `await` キーワードが使用可能となる. 

```rust
async fn example(min_len: usize) -> String {
    let content = async_read_file("foo.txt").await;
    if content.len() < min_len {
        content + &async_read_file("bar.txt").await
    } else {
        content
    }
}
```

`example` 関数は上で紹介した combinator 関数を `async`/`await` を使ったものに変更したもの. 
`.await` 命令を使うことで, future の値をクロージャや `Either` 型を使うことなく取ってこれる. 
結果として, 非同期なコードを通常の同期的なコードのように書くことが可能である. 

#### State Machine Transformation

コンパイラが水面下で行っていることは, `async` 関数の中身を状態機械に変換することで, 各 `.await` 呼び出しで別の状態を表現している. 
上記の `example` 関数では, コンパイラは次の 4つの状態をもつ状態機械をつくり出している: 
- Start
- Waiting on foo.txt
- Waiting on bar.txt
- End

各状態は関数の一時停止ポイントを表現している.
Start と End 状態は関数実行の最初と最後を表現している. 
Waiting on foo.txt 状態は関数が最初の `async_read_file` の結果を待っている状態を表現している. 
同様に, Waiting on bar.txt 状態は 二度目の `async_read_file` の結果を待っている状態を表現している. 

状態機械は 各 `poll` 呼び出しを状態遷移の可能性にすることで `Future` トレイトを実装する. 

```
 ┌─────┐
 │Start│
 └──┬──┘
    │
    ▼
┌────────┐no ┌──────────┐
│foo.txt ├──►│Waiting on│
│ ready? │   │ foo.txt  │
└───┬────┘   └─┬────────┘
    │   ▲      │
yes │   └──────┘
    │      poll()
    ▼
┌────────┐no ┌──────────┐
│bar.txt ├──►│Waiting on│
│ ready? │   │ foo.txt  │
└───┬────┘   └─┬────────┘
    │   ▲      │
    │   └──────┘
    │      poll()
    ▼
 ┌─────┐
 │ End │
 └─────┘
```
#### Saving State

最後の待機状態から継続できるようにするため, 状態機械は現在の状態を内部的に記録しておく必要がある. 
加えて, 次の `poll` 呼び出し時に実行を継続するのに必要な変数すべてを保存しておく必要もある. 
ここでコンパイラが活躍することになる: コンパイラはどの変数がいつ使用されるかを知っているため, 必要な変数に応じて構造体を自動で生成することが可能である. 

```rust
async fn example(min_len: usize) -> String {
    let content = async_read_file("foo.txt").await;
    if content.len() < min_len {
        content + &async_read_file("bar.txt").await;
    } else {
        content
    }
}

// コンパイラが生成する構造体

// Start 状態
struct StartState {
    min_len: usize, // content.len() との比較に必要なので保存
}

// Waiting on foo.txt 状態
struct WaitingOnFooTxtState {
    min_len: usize,
    foo_txt_future: impl Future<Output = String>, // async_read_file() の future を保存. 再 poll 時に必要となる
}

// Waiting on bar.txt 状態
struct WaitingOnBarTxtState {
    content: String,
    bar_txt_future: impl Future<Output = String>,
}

// End 状態
struct EndState {}
```

#### The Full State Machine Type

生成された状態機械が `example` 関数に対してどのように見えるかを想像することは、理解の助けとなる.
異なる状態を表現する構造体を定義し, 必要な変数をそこに保存した. 
そこに状態機械を作るために、それらの構造体を `enum` にまとめる: 
```rust
enum ExampleStateMachine {
    Start(StartState),
    WaitingOnFooTxt(WaitingOnFooTxtState),
    WaitingOnBarTxt(WaitingOnBarTxtState),
    End(EndState),
}
```

各状態について別の enum variant (ここでは enum の内部の型) を定義し, 対応する状態の構造体を各 variant にフィールドとして加えた. 
状態遷移を実装するため, コンパイラは `Future` trait の implementation を `example` 関数の挙動を基にして行う: 


```rust
impl Future for ExampleStateMachine {
    type Output = String; // return type of `example`

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        loop { // loop は状態を進めるときに使われる. 各状態での処理で Poll::Pending (or Poll::Ready)が出たときは return する. 
            match self { // TODO: `Pin` を処理する
                ExampleStateMachine::Start(state) => {…}
                ExampleStateMachine::WaitingOnFooTxt(state) => {…}
                ExampleStateMachine::WaitingOnBarTxt(state) => {…}
                ExampleStateMachine::End(state) => {…}
            }
        }
    }
}
```

```rust
ExampleStateMachine::Start(state) => {
    // from body of `example` // `example` 関数の処理
    let foo_txt_future = async_read_file("foo.txt");
    // `.await` operation // 状態の変更
    let state = WaitingOnFooTxtState {
        min_len: state.min_len,
        foo_txt_future,
    };
    *self = ExampleStateMachine::WaitingOnFooTxt(state);
}
```

```rust
ExampleStateMachine::WaitingOnFooTxt(state) => {
    match state.foo_txt_future.poll(cx) { // poll して
        Poll::Pending => return Poll::Pending, // Pending を return
        Poll::Ready(content) => { // Ready なら処理を進める
            // from body of `example` 
            if content.len() < state.min_len {
                let bar_txt_future = async_read_file("bar.txt");
                // `.await` operation // 状態の変更
                let state = WaitingOnBarTxtState {
                    content,
                    bar_txt_future,
                };
                *self = ExampleStateMachine::WaitingOnBarTxt(state);
            } else {
                // 状態を変更
                *self = ExampleStateMachine::End(EndState));
                // Ready を return 
                return Poll::Ready(content);
            }
        }
    }
}
```

```rust
ExampleStateMachine::WaitingOnBarTxt(state) => {
    match state.bar_txt_future.poll(cx) { // poll して
        Poll::Pending => return Poll::Pending, // Pending なら return
        Poll::Ready(bar_txt) => { // Ready なら処理を進める
            // 状態の変更
            *self = ExampleStateMachine::End(EndState));
            // from body of `example` // 処理を進める (ここでは return)
            return Poll::Ready(state.content + &bar_txt);
        }
    }
}
```

```rust
ExampleStateMachine::End(_) => {
    // `Ready` を return しているはずなのでここについたら panic
    panic!("poll called after Poll::Ready was returned");
}
```

これでコンパイラによって生成された状態機械と `Future` trait の implementation がどのように見えるかがわかった. 
実践では, コンパイラは別の方法でコードを生成する. 
(もし興味があれば, 実装は現在 [generators](https://doc.rust-lang.org/nightly/unstable-book/language-features/generators.html) に基づいてるが, これは実装上の詳細だけである) 

パズルの最後のピースは `example` 関数それ自身によって生成されるコードである. 
関数のヘッダは次のように定義される: 
```rust
async fn example(mil_len: usize) -> String
```

関数の中身はもう状態機械によって実装されてるので, この関数がしなければならないことは状態機械の初期化とそれを返すことである. 

```rust
fn example(min_len: usize) -> ExampleStateMachine {
    ExampleStateMachine::Start(StartState {
        min_len,
    })
}
```

完成!

TODO: `ExampleStateMachine` を受け取るのは誰? -> たぶん executor (executor が future 状態機械を受け取り (もしくはヒープ上にあるそれのポインタを持っていて), 操作する)

### Pinning

pin について見ていく.

#### Self-Referential Struct
状態機械の遷移は各一時停止ポイントのローカル変数は構造体に保存される. 
`example` 関数のように小さい例の場合は, 単純明快でなんの問題も引き起こされなかった. 
しかし, 変数が相互に参照しあっている場合はもっと難しくなる. 

```rust
async fn pin_example() -> i32 {
    let array = [1, 2, 3];
    let element = &array[2];
    async_write_file("foo.txt", element.to_string()).await;
    *element
}
```

このような関数に対して, waiting on write 状態は以下のようになる: 
```rust
struct WaitingOnWriteState {
    array: [1, 2, 3],
    element: 0x1001c, // array[2] の先頭アドレス
}
```
`element` は返り値であるため, また `array` は `element` に参照されているため, `array` と `element` の両方の変数を保存する必要がある. 
`element` は参照なので参照先の要素へのポインタ (メモリアドレス) を保存する. 
例としてメモリアドレスを `0x1001c` とする. 
実際には `array` の最後の要素のアドレスが必要なので, 構造体がどこに保存されるかに依存することになる. 
このような内部ポインタをもつ構造体は自己参照構造体と呼ばれ, それらは自身のフィールドの一つから参照されている. 

#### The Problem with Self-Referential Structs

自己参照構造体の内部ポインタは根本的な問題につながっている. 

構造体のメモリ上の位置が変更されると, ポインタがダングリングして次の `poll` で未定義動作を引き起こしてしまう. 

### Possible Solutions

MEMO: ここでの move は関数へ引数を渡すなど構造体インスタンスのメモリアドレスが変更されるような移動のこと

---
TODO: Rust の所有権の移転としての move と, このメモリ上の移動としての move は同一概念? (所有権 move <=> メモリ move が成り立つ?) 

-> Rust の所有権はメモリの解放責任を負うためのもの (= メモリ安全性を担保する概念) なので, 違う概念.

> Because variables are in charge of freeing their own resources, resource can only have one owner. This also prevents resources from being freed more than once. 

[Ownership and moves](https://doc.rust-lang.org/rust-by-example/scope/move.html) より

---

このダングリングポインタ問題を解決する手法が 3つある. 
- 構造体の move 時にポインタを更新する: この手法はパフォーマンスを犠牲にした Rust への拡張的な変更が必要となりる. ランタイムが全ての型のフィールドを追跡し, すべての move 命令でポインタが更新されたか確認することが必要となる. 
- 自己参照の代わりにオフセットを保存する: ポインタの更新を避けるため, コンパイラは自己参照を構造体の先頭からのオフセットとして代わりに保存しようとする可能性がある. この手法の問題点はコンパイラがすべての自己参照を検知する必要があることである. これはユーザーの入力に依存することもあるのでコンパイル時には不可能であり, 結局 ランタイムシステムが必要になる. 
- 構造体の move を禁止する: 上で見た通り, ダングリングポインタはメモリ内の構造体を move させるときにのみ発生する. 自己参照構造体に対して move 命令を完全に禁止することで, 問題は回避される. この手法の大きな利点は, この手法が型システムで実装でき, ランタイムを必要としない点である. 欠点は move 命令への制限を自己参照構造体となりうるに対してプログラマが対応しなければならない点である. 

ゼロコスト抽象の原則のため, Rust は第三の選択肢を選んだ. 
このために pinning API が RFC 2349 で提案された. 
以下では, この API についての概要とそれが async/await や futures とどのように連携するかを説明する. 

#### Heap Values
最初の観察は heap allocated values がほとんどの場合すでに固定のメモリアドレスを保持していることだ. 
それらは `allocate` 呼び出しによって生成され, `Box<T>` 等のポインタ型として参照される. 
このポインタ型は move が可能であるが, ポインタが指している heap の値は `deallocate` によって解放されるまで常に同じメモリアドレスにとどまっている. 

MEMO: スタックにある値は返り値になって他のスタックに移動したり, もしくはスタックの解放で消滅したりする. 一方でヒープ上の値は `deallocate` されるまで常に同じメモリアドレスをもつ. 

heap allocation を使って自己参照構造体を作成する. 

```rust
fn main() {
    let mut heap_value = Box::new(SelfReferential {
        self_ptr: 0 as *const _,
    });
    let ptr = &*heap_value as *const SelfReferential;
    heap_value.self_ptr = ptr;
    println!("heap value at: {:p}", heap_value);
    println!("internal reference: {:p}", heap_value.self_ptr);
}

struct SelfReferential {
    self_ptr: *const Self,
}
```

このコードを実行すると, heap の値のアドレスと内部ポインタのアドレスが等しくなっていることがわかる. 
つまり, `self_ptr` フィールドが有効な自己参照であることを意味している. 
`heap_value` 変数はポインタのみなので, それを move させることは構造体のアドレス自体を変えるものでなく, 従って `self_ptr` はポインタが move したとしても有効でありつづける. 

しかし, この例であっても破壊されうる: `Box<T>` の外に move して中身を入れ替えることが可能である. 
```rust
let statck_value = mem::replace(&mut *heap_value, SelfReferential {
    self_ptr: 0 as *const _,
});
println!("value at {:p}", &stack_value);
println!("internal reference: {:p}", stack_value.self_ptr);
```

`mem::replace` を使うことで heap allocated value を新しい構造体インスタンスで入れ替えている. 
これによって元の `heap_value` がスタックへと move されるが, `self_ptr` フィールドはダングリングポインタとなってしまう. 
したがって, ヒープアロケーションは自己参照を安全にするのに十分とは言えない. 

#### `Pin<Box<T>>` and `Unpin`

pinning API は `&mut T` 問題の解決策を `Pin` wrapper 型と `Unpin` marker trait の形で提供している. 
これらの型の背景にある考え方は, `Unpin` trait で wrap された値 (e.g. `get_mut`, `defef_mut`) への `&mut` 参照を得るために使われる `Pin` メソッドの全てをゲートすることにある (?) 

`Unpin` trait は auto trait で, 明示的に opt-out した型以外の全ての型に自動ですでに実装されている. 
自己参照構造体で `Unpin` を opt-out することで, `Pin<Box<T>>` 型から `&mut T` を得る (safe な) 方法がなくなる. 
結果として内部の自己参照が有効であると保証される. 

TODO: `Unpin` であるなら `mem::replace` 等のインスタンスの**メモリ上**の move ができるという理解でいい? それを `Pin` で opt-out することで move を不可能にするということ? > そう

MEMO: 自己参照構造体への `&mut` 参照を禁止したい (なぜなら `mem::replace` されるから). 

MEMO: 自己参照構造体の安全を保つ策: 1. ヒープに値を置く, 2. `mem::replace` をさせないように `&mut` 参照を禁止する. 

##### 例

例として, `SelfReferential` の `Unpin` を opt-out する. 
```rust
use core::marker::PhantomPinned;

struct SelfReferential {
    self_ptr: *const Self,
    _pin: PhantomPinned,
}
```
`PhantomPinned` 型の `_pin` フィールドを追加することで opt-out する. 
この型は 0サイズのマーカー型で `Unpin` trait を実装しないことがただ一つの目的. 
auto traits が働いているため, このフィールドだけでは `Unpin` を完全に opt-out にはまだ足りない. 

第二のステップは例えば `Box<SelfReferential>` を `Pin<Box<SelfReferential>>` 型へと変更すること. 
もっとも簡単な方法は heap allocated value をつくるのに `Box::new` の代わりに `Box::pin` 関数を使うこと. 

```rust
let mut heap_value = Box::pin(SelfReferential {
    self_ptr: 0 as *const _,
    _pin: PhantomPinned,
});
```

`Box::new` を `Box::pin` へと変更するのに加え, `_pin` フィールドを構造体初期化に加える必要がある. 
`PhantomPinned` は 0サイズ型なので, 初期化にはその型名を置くだけでよい. 

この例を走らせようとすると失敗する: 
```
error[E0594]: cannot assign to data in a dereference of `std::pin::Pin<std::boxed::Box<SelfReferential>>`
  --> src/main.rs:10:5
   |
10 |     heap_value.self_ptr = ptr;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^ cannot assign
   |
   = help: trait `DerefMut` is required to modify through a dereference, but it is not implemented for `std::pin::Pin<std::boxed::Box<SelfReferential>>`

error[E0596]: cannot borrow data in a dereference of `std::pin::Pin<std::boxed::Box<SelfReferential>>` as mutable
  --> src/main.rs:16:36
   |
16 |     let stack_value = mem::replace(&mut *heap_value, SelfReferential {
   |                                    ^^^^^^^^^^^^^^^^ cannot borrow as mutable
   |
   = help: trait `DerefMut` is required to modify through a dereference, but it is not implemented for `std::pin::Pin<std::boxed::Box<SelfReferential>>`
```

どちらのエラーも `Pin<Box<SelfReferential>>` 型が `DerefMut` trait を実装していないために発生する. 
これは `DerefMut` trait が `&mut` 参照を返しうる (`&mut` 参照をつくるには `DerefMut` が必要となる) ことを考えるとこれは好ましいこと. 
これは `Unpin` を opt-out して, `Box::new` を `Box::pin` へと変更するだけで起こる. 

 問題は, コンパイラが `self_ptr` フィールドの初期化まで禁止してしまうこと. 
 これはコンパイラが `&mut` 参照の有効な使い方と無効な使い方を差別化できないために起こる. 
 初期化を機能させるため, unsafe な `get_unchecked_mut` メソッドを使う. 

 ```rust
 unsafe {
     let mut_ref = Pin::as_mut(&mut heap_value);
     Pin::get_unchecked_mut(mut_ref).self_ptr = ptr;
 }
 ```

`get_unchecked_mut` 関数は `Pin<Box<T>>` の代わりに `Pin<&mut T>` で機能し, 従って `Pin::as_mut` を使う必要がある. 
`get_unchecked_mut` によって返される `&mut` 参照を使って `self_ptr` フィールドをセットできる. 

いま, 残されたエラーは `mem::replace` に関する望まれるエラーだけになった. 
この操作は heap allocated value をスタックへと move しようとするものであり, それによって `self_ptr` フィールドに保存された自己参照を破壊するものである. 
`Unpin` を opt-out して `Pin<Box<T>>` を使うことで, この操作をコンパイル時に防ぐことが可能となり,  ゆえに自己参照構造体を安全に用いることができる. 
コンパイラは自己参照の作成の安全性を証明することができないため, unsafe ブロックとして自分自身でその適切さを検証する必要がある. 

#### Stack Pinning and `Pin<&mut T>`
前のセクションでは heap に allocate された自己参照な値を安全につくるために `Pin<Box<T>>` をどのように使うかを学んだ. 
この手法はうまく機能し (unsafe による初期化以外は) 比較的安全であるが, 必要とされる heap allocation にはパフォーマンスコストが付随する. 
Rust はゼロコスト抽象を提供したいため, pinning API は stack allocated value を指す `Pin<&mut T>` インスタンスをつくることを可能にしている. 

`Pin<Box<T>>` (wrap された値の所有権を持っている) とは異なり, `Pin<&mut T>` インスタンスは wrap された値を借用しているだけ. 
これによって事情はさらに複雑になり, プログラマが追加の保証をすることが必要となる. 
最も重要なのは, `Pin<&mut T>` が参照されている `T` の生存期間ずっと pin され続けれていなければならないことであり, これを stack based variables に検証することは難しい. 
これを容易にするため `pin-utils` のようなクレートもあるが, stack 変数を pin することはおすすめしない. 


#### Pinning and Futures

`Future::poll` メソッドには `Pin<&mut Self>` が引数として使われている: 
```rust
fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>
```

通常の `&mut self` ではなく `self: Pin<&mut Self>` が使われている理由は, async/await からつくられた future インスタンスはしばしば自己参照になる (async 関数で参照を用いることで自己参照が発生する可能性がある) からである. 
`Self` を `Pin` に wrap して async/await から生成された自己参照 futures のためにコンパイラに `Unpin` を opt-out してもらうことで, futures が `poll` 呼び出しの間にメモリ上で移動することがなくなる. 
これによってすべての内部参照が有効であることが保証される. 

### Executors and Wakers
async/await を使うと, 人間的に futures を扱うことが可能となる. 
しかし, 上で学んだ通り, futures は poll されない限り何もしない. 
つまり, どこかの時点で `poll` を呼び出す必要があり, でなければ非同期コードは決して実行されない. 

一つの future であれば, loop を用いて各 future を待つことが可能となる. 
しかし, この手法は非常に非効率的で多くの futures をつくるプログラムには有効でない. 
この問題に対する最も普通の答えは, システムの全ての futures を終了するまで poll する global executor を定義すること. 

#### Executors

executor の目的は独立したタスクとして futures を spawn することを可能にすることで, それは `spawn` メソッドのようなものを通して行われる. 
中央集権的にすべての futures を管理する利点は future が `Poll::Pending` を返すときに別の future へと切り替えることができること. 
つまり 非同期操作が並列 parallel に走り, CPU が busy に保たれる (= CPU 時間が無駄にならない). 

多くの executor の実装は複数の CPU コアのあるシステムを利用している. 
それらは全てのコアを使用することのできる thread pool をつくり, またコア同士の付加をバランスする work stealing などの技術を使う. 
また, 組み込みシステム用の, 遅延とメモリオーバーヘッドを小さく最適化した executor 実装もある. 

futures を poll するオーバーヘッドを回避するため, executors は Rust の futures がサポートする waker API を利用することが多い. 

#### Wakers

waker API の背後にある考え方は, 特別な `Waker` 型が `Context` 型で wrap されて各 `poll` 呼び出しに渡されるというものである. 
この `Waker` 型は executor によってつくられ, 非同期タスクでタスクの完了をシグナルするのに使用される. 
結果的に, executor は対応する waker に通知されるまで以前に `Poll::Pending` を返した future の `poll` を呼び出す必要がなくなる. 

小さい例で説明するのがよい: 
```rust
async fn write_file() {
    async_write_file("foo.txt", "Hello").await;
}
```

この関数は非同期的に文字列 "Hello" を `foo.txt` へと書き込む. 
ハードディスクへの書き込みは時間がかかるので, この future に対する最初の `poll` 呼び出しは `Poll::Pending` が返るだろう. 
しかし, ハードディスクドライバは `poll` 呼び出しで渡された `Waker` を内部で保存し, ファイルがディスクへと書き込まれたときに, その `Waker` を使って executor にそのことを知らせる. 
この方法だと, executor は waker からの通知を受け取るまで `poll` をしようとして時間を無駄にすることがない. 

### Cooperative Multitasking?

futures と async/await は cooperative multitasking パターンの実装である: 

- executor に追加される各 future は基本的に協調的タスクである. 
- 排他的な yield 操作を使うのではなく, futures が `Poll::Pending` を返すことで CPU コアの制御を明け渡すこと. 
    - futures に CPU の明け渡しを強制をするものはない. そうしたいなら `poll` を返さないこともできる. 
    - 各 future が別の future の実行をブロックできるため, futures が悪意のあるものでないと信頼する必要がある. 
- futures は内部で次の `poll` 呼び出しで実行を続けるのに必要な状態をすべて保存する. async/await では, コンパイラは自動的に必要な変数をすべて検知して生成された状態機械に保存する. 
    - 実行に必要な最低限の状態だけが保存される.  
    - `poll` メソッドはリターン時に call stack を放棄するので, 同じスタックが他の future の poll に使われる. 

future と async/await が協調的マルチタスキングのパターンにあっていることを確認した. 

## Implementation

`Future` trait は `core` ライブラリの一部で, async/await は言語の機能そのものなので, `#![no_std]` 環境のカーネルで使うのに, 特別なことは必要ない. 

`src/main.rs`
```rust
async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
}

```

`example_task` によって返された future を走らせるには, `Poll::Ready` が返されるまで `poll` を呼び出す必要がある. 
これを行うためには, 単純な executor 型を作成する必要がある.

TODO: future はあるけど executor はつくらないといけない? なぜ?


### Task

executor の実装の前に, `task` モジュールと `Task` 型を作成する必要がある. 

`src/lib.rs`: 
```rust
pub mod task;
```

`src/task/mod.rs`: 
```rust
use core::{future::Future, pin::Pin};
use alloc::boxed::Box;

pub struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
}
```

`Task` 構造体は pin され, ヒープにアロケートされ, そして動的ディスパッチされた future で空の型 `()` を output とする newtype wrapper. 

- `()` を返す task と連携する future が必要となる. つまり task は何の結果も返さない, ただ副作用で実行される. 例えば, `example_task` 関数は何の値も返さないが, side effect として画面に何かをプリントする.  
- `dyn` キーワードはトレイトオブジェクトを `Box` に保存することを示す. これはつまり future のメソッドが動的にディスパッチされているということで, 動的ディスパッチは `Task` 型の内部の異なった future 型を保存することを可能にする. これは各`async fn` がそれぞれの型を持ち, 複数の異なった task を作成できるようにしたいために重要.  
- `Pin<Box>` 型はヒープ上で値がメモリ上で移動することがないように保証し, また `&mut` 参照をつくることを防ぐ (`mem::replace` などによる中身の移転を防ぐ). async/await で生成された futures が self-referential であること可能性があるため. 

TODO: まだ `Task` と `Pin` の関係, なぜ必要なのかを納得できていない. `Pin` は future から作られる状態機械の自己参照を守るため, という認識でよい? > OK

futures から `Task` 構造体をつくる `new` 関数を定義する. 

`src/task/mod.rs`: 
```rust
impl Task {
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task {
            future: Box::pin(future), // `Pin<Box>` をつくる
        }
    }
}
```

`Task` は任意の時間まで生存するので future も同じ時間だけ有効である必要があり, `'static` lifetime が必要. 

executor が保存された future を poll するための `poll` メソッドを追加する. 
`src/task/mod.rs`: 
```rust
use core::task::{Context, Poll};

impl Task {
    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future
            .as_mut() // `Pin<&mut T>` をとってくる (`&mut T` ではない)
            .poll(context) // poll (引数に `self: Pin<&mut Self>` をとる)
    }
}
```

### Simple Executor

`src/task/mod.rs`: 
```rust
pub mod simple_executor;
```

`src/task/simple_executor.rs`: 
```rust

use super::Task;
use alloc::collections::VecDeque;

pub struct SimpleExecutor {
    task_queue: VecDeque<Task>, // FIFO queue
}

impl SimpleExecutor {
    pub fn new() -> SimpleExecutor {
        SimpleExecutor {
            task_queue: VecDeque::new(),
        }
    }

    pub fn spawn(&mut self, task: Task) {
        self.task_queue.push_back(task)
    }
}
```

#### Dummy Waker

`poll` メソッドを呼び出すため, `Context` 型を作成する必要があり, `Context` は `Waker` 型を wrap する. 
最初は単純のため何もしない dummy waker をつくる. 
そのためには, `core::task::RawWaker` インスタンスをつくり (`RawWaker` は別の `Waker` メソッドの実装を定義する), そして `Waker::from_raw` 関数を使って `RawWaker`を `Waker` へと変える. 

`src/task/simple_executor.rs`: 
```rust
use core::task::{Waker, RawWaker};

fn dummy_raw_waker() -> RawWaker {
    todo!();
}

fn dummy_waker() -> Waker {
    unsafe { Waker::from_raw(dummy_raw_waker())} // プログラマが責任を持つ必要があるため unsafe. `RawWaker` はドキュメントに記載された要件を定めなければ未定義動作が起こりうる. 
}
```

`from_raw` 関数はプログラマが `RawWaker` のドキュメント記載の要件を満たさないと未定義動作を引き起こすので unsafe. 
`dummy_raw_waker` 関数の実装を見る前に, `RawWaker` 型がどのように働くか理解する. 

##### `RawWaker`
`RawWaker` 型はプログラマが `RawWaker` が clone, wake, drop されたときに呼ばれるべき関数を定義した virtual method table (vtable) を定義することを要求する. 
この vtable のレイアウトは `RawWakerVTable` 型で定義される. 
各関数が基本的に型情報を消したポインタである (そうみなせる) `*const ()` 引数を受け取る (例えば, ヒープにある構造体へのポインタ). 
適切な参照ではなく `*const ()` ポインタを使っている理由は `RawWaker` 型が非ジェネリクスかつ任意の型をサポートする必要があるから. 
関数に渡されるこのポインタ値は `RawWaker::new` へ与えられる `data` ポインタである.

典型的には `RawWaker` は `Box` や `Arc` で wrap されてヒープにアロケートされた構造体もののためにつくられる. 
そのような型には, `Box::into_raw` が `Box<T>` を `*const T` ポインタへの変換するのに使える. 
このポインタは匿名ポインタ `*const ()` へとキャストして `RawWaker::new` へと渡される. 
各 vtable 関数も同じ `*const ()` を引数として受け取るので, 各関数はポインタを `Box<T>`, `&T` へとキャストし戻すことができる (TODO: 本当に?). 
このプロセスは非常に危険で簡単に未定義動作となる. 
このため, 必要でない限り手動で `RawWaker` を作成するのはおすすめできない. 
    
---
参考: `RawWaker::new` のヘッダは 
```rust
pub const fn new(data: *const (), vtable: &'static RawWakerVTable) -> RawWaker
```

TODO: `*const ()` について
- `*const` は生ポインタ
- `*const u8` で 1バイトの領域へのポインタを表現している 
- `*const u64` で 8バイトの領域へのポインタを表現している (e.g. ページテーブルのエントリ) 
- `*const ()` で空の構造体 (= 型情報のない) ポインタを表現している 


TODO: vtable について理解できていない. なぜ必要? 

vtable については `core::task::RawWakerVTable` を使っている. 

vtable の初期化には `RawWakerVTable::new` が使われる.
```rust
pub const fn new(
    clone: unsafe fn(_: *const ()) -> RawWaker,
    wake: unsafe fn(_: *const ()),
    wake_by_ref: unsafe fn(_: *const ()),
    drop: unsafe fn(_: *const ())
) -> Self
```

関数を渡して初期化している. トレイトっぽいことをやっている?



> `RawWaker` type should be non-generic but still support arbitrary types 

Rust のジェネリクスでは対応できない -> vtable を用いた多相性 (?)

TODO: なぜジェネリクスにできない? 

たぶん `*const ()` と vtable はセットで考える. 
- `*const ()` は匿名のポインタ (ここではヒープにある future trait 実装型へのポインタ? それとも executor へのポインタ?). 
- ``

TODO: Waker ってどううごいている? そこが想像できていない?

参照: 
- [`core::task::RawWakerVTable`](https://doc.rust-lang.org/stable/core/task/struct.RawWakerVTable.html) 

---

##### A Dummy `RawWaker`

`RawWaker` を手動でつくることは推奨されないものの, なにもしない dummy `Waker` をつくる方法もない. 
幸運なことに, dummy `Waker` 何もさせないことによって `dummy_raw_waker` 関数を比較的安全に実装することが可能になる. 

`src/task/simple_executor.rs`: 
```rust
use core::task::RawWakerVTable;

fn dummy_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {} // 何もしない
    fn clone(_: *const ()) -> RawWaker {
        dummy_raw_waker() // clone するときはこの関数を呼ぶような関数
    }

    let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(0 as *const (), vtable)
}
```

TODO: まったく関係ないが, 引数として関数を渡す場合, メモリはどのように使用される? 

#### A `run` Method
`Waker` インスタンスをつくる必要があり, `run` を実装する. 



















