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

futures はまだ利用可能でない値を表現している (future は日本語だと未来という意味になりがちだが, 「先物(取引)」という意味もある). 
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

TODO: `Waker` からのシグナル "`Future` is ready!"-> `poll` で結果を確認する. この流れであってる? > あとで

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

TODO: future を future のまま計算する

```rust
// 実際には動かないコード

struct StringLen<F> {
    inner_future: F,
}

impl<F> Future for StringLen<F> 
where F: Future<Output = String> {
    type Output = usize;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
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

future combinators が効率的なコードを可能にする一方で, その型やクロージャを用いたインターフェイスのために使いにくい状況もある. 

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





TODO: この章にはまったく関係ないが, 通常の OS では入れ子の関数はどのようなスタックを持つ?
























