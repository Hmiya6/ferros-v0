[VGA Text Mode](https://os.phil-opp.com/vga-text-mode/)

# VGA Text Mode の一次メモ

VGA テキストモードは文字をスクリーンにプリントする簡単な方法.

## The VGA Text Buffer

典型的な VGA テキストバッファは 25行x80列の 2次元配列.

VGA text buffer's format:
```
bits: value
0-7: ASCII code point (8 bits = 1 byte)
8-11: Foreground color (4 bits) // 色については元記事参照
12-14: Background color (3 bits)
15: Blink (1 bit)
```

VGA テキストバッファは `0xb8000` への memory-mapped I/O によってアクセス可能. つまり, そのアドレスへの読み書きは RAM のアクセスではなく, VGA ハードウェア上のテキストバッファに直接行われる. 

---
### 補足: memory-mapped I/O について
概要
> メモリマップドI/Oは、アドレス空間（仮想記憶方式の場合、物理アドレス空間）上にメモリと入出力機器が共存し、メモリのリード/ライトのためのCPU命令を入出力機器にも使用する。

例
> 8ビットマイクロプロセッサを使った単純なシステムを例として説明する。アドレス線が16ビット分あれば、64Kバイトまでのメモリをアドレス指定可能である。アドレス空間の先頭32KバイトにRAMを配置し、空間の最後尾16KバイトにROMを配置する。残った中間の16Kバイトの空間を各種入出力機器に割り当てる（タイマ、カウンタ、ビデオディスプレイチップ、サウンドジェネレータなど）。

いずれも [メモリマップド I/O - Wikipedia](https://ja.wikipedia.org/wiki/%E3%83%A1%E3%83%A2%E3%83%AA%E3%83%9E%E3%83%83%E3%83%97%E3%83%89I/O) より

---

メモリにマップされた I/O 機器と, 通常のメモリではその扱い方が異なる場合もある (通常の read/write ができない可能性もある). 

また, コンパイラはメモリにマップされた I/O と通常のメモリの区別がつかない.

## A Rust Module

```rust
// in src/vga_buffer.rs

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)] // <- メモリ上での保存方法を指定 (`u8` として保存)
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)] // <- transparent でメモリレイアウトを `u8` と同じにする ()
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}
```



---
### 補足: `#[repr(...)]` とレイアウトについて
> By default, as you'll see shortly, the Rust compiler gives very few guarantees about how it lays out types, ... Luckily, Rust provides a `repr` attribute you can add to your type definitions to request a particular in-memory representation for that type. ...

デフォルトでは, Rust のコンパイラは型をどのように並べる (lay out) かほとんど保証しない. ... Rust には `repr` 属性を型宣言につけ, その型に特定の in-memory representation を要請することができる. 

例:
```rust
#[repr(C)]
struct Foo {
    tiny: bool,
    normal: u32,
    small: u8,
    long: u64,
    short: u16,
}
```
まずコンパイラは `tiny` を見る, そのサイズは 1 bit. byte alignment により, 1 byte の in-memory representation が与えられる. 
次に `normal` は 4 byte の型なので, `Foo` のいままでのメモリを 4-byte-aligned にしたいが, `tiny` が 1-byte-aligned なので alignment を誤ってしまう (こんな感じ: ` tiny (1 byte) + normal (4byte) = 5 bytes`). 
そのため, コンパイラは 3 byte をパディング padding として `tiny` と `normal` の間に入れる (`tiny (1) + padding (3) + normal (4) = 8`). 

`small` は 1 byte で, 今までの bytes は 8 byte で byte-aligned. なので, `small` はそのまま `normal` の後に続けて入れる (`1 + (3) + 4 + 1 = 9`). 
`long` を入れる前に `Foo` のメモリを 8-byte-aligned にしておきたいので, 7 byte パディングする. (`1 + (3) + 4 + 1 + (7) + 8 = 24`).
`short` は 2-byte-aligned で, `Foo` も 24 byte なので, そのまま `short` を後ろにおいて, `Foo` のサイズは 26 byte となる.

```rust
// size = 26
#[repr(C)]
struct Foo {
    tiny: bool,
    normal: u32,
    small: u8,
    long: u64,
    short: u16,
}

// size = 16
#[repr(C)]
struct Foo1 {
    long: u64,
    normal: u32,
    short: u16,
    small: u8,
    tiny: bool,
}
```
2つの構造体が意味するものは違う.

...

Rust for Rustacean (書籍) より

---

### Text Buffer

出来上がったのがこれ:
`src/vga_buffer.rs`
```rust
// * snip *

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer, // VGA text buffer lives for entire runtime
}

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;
                self.buffer.chars[row][col] = ScreenChar {
                    ascii_character: byte,
                    color_code,
                };
                self.column_position += 1;
            }
        }
    }

    fn new_line(&mut self) {
        todo!();
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // printable ASCII byte or newline
                0x20..=0x7e | b'n' => self.write_byte(byte),
                // not part of printable ASCII range
                _ => self.write_byte(0xfe), // `0xfe` represents `■` on VGA.
            }
        }
    }

}

pub fn print_something() {
    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::DarkGray),
        buffer: unsafe {&mut *(0xb8000 as *mut Buffer)}, // `Buffer` への生ポインタの参照外しの可変参照.
    };

    writer.write_byte(b'H');
    writer.write_string("ello World!");
    writer.write_string("こんにちは!");

}

```

TODO: `bootloader` 0.10~ では, 動かない可能性がある. ~0.9 に戻して実装する?  
-> した


### Volatile

この方法は Rust コンパイラの最適化によって機能しなくなる可能性がある. 

問題は `Buffer` に書き込むだけで読み込みをしないこと. 
コンパイラには (通常の RAM にアクセスするのか VGA buffer メモリにアクセスするのか判別できない. 
したがって, コンパイラは書き込みを不要とみなして省略する可能性がある. 
この最適化を回避するため, **これらの書き込みは volatile としなければならない**. 
これによって書き込みには副作用があり, 最適化されるべきでないことをコンパイラに伝えることができる. 

VGA buffer への volatile write を使うため, `volatile` ライブラリを使う. 
このクレートは `read` `write` メソッドを持つ `Volatile` ラッパーを提供する.


`Cargo.toml`
```toml
[dependencies]
volatile = "0.2.6"
```

`0.2.6` でないと動かない. 

`src/vga_buffer.rs`:
```rust
use volatile::Volatile;

struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}
```

```rust
impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {

                // * snip *

                self.buffer.chars[row][col].write(ScreenChar { // `write` は `Volatile` で提供される 
                    ascii_character: byte,
                    color_code,
                });
                
                // * snip *
            }
        }
    }
    
    // * snip *

}
```

### Formatting macros
コードを書くだけなので省略
### Newlines
コードを書くだけなので省略

## A Global Interface
`Writer` インスタンスを持ち出さなくても他のモジュールからインターフェースとして使える global writer  がほしい -> static な `WRITER` をつくる.

```rust
// in src/vga_buffer.rs

pub static WRITER: Writer = Writer {
    column_position: 0,
    color_code: ColorCode::new(Color::Yellow, Color::Black),
    // `Buffer` への生ポインタの参照外しの可変参照
    // 
    // `0xb8000 *mut Buffer` で生ポインタの開始アドレスとして宣言
    // `*(0xb8000 *mut Buffer)` で生ポインタの参照外し
    buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
};

```

これはコンパイルできない. 

エラーコード: 
```
error[E0015]: calls in statics are limited to constant functions, tuple structs and tuple variants
   --> src/vga_buffer.rs:132:17
    |
132 |     color_code: ColorCode::new(Color::Yellow, Color::Black),
    |                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error[E0658]: dereferencing raw pointers in statics is unstable
   --> src/vga_buffer.rs:133:21
    |
133 |     buffer: unsafe {&mut *(0xb8000 as *mut Buffer)},
    |                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
```

statics はコンパイル時に初期化 initialize される (通常の変数は run time で初期化される): "const evaluator" (簡単にいえば, const 値をコンパイル時に計算処理しておくこと. ). 

`ColorCode` のエラーは constant 関数 によって解決可能. しかし根本的な問題は Rust の const evaluator はコンパイル時に生ポインタを参照に変換できないことにある (今後解決されると思われる). 

---
TODO: const evaluator について, Rust のそれについて
TODO: constant function: コンパイル時に実行される関数?

---

### Lazy Statics
> The one-time initialization of statics with non-const functions is a common problem in Rust.

非 const 関数で statics を一度だけしか初期化しないのは Rust のよくある問題. 

---
QUESTION: どういう意味?
A: 
> 設定ファイルや辞書データなどの適当なファイルをあらかじめ読んでおき、その内容をグローバルに置きたい、という場面はそれなりにあると思います。Rust では、static でグローバル変数をつくることができますが、初期化に使う式はコンパイル時に評価できるものでなくてならないため、先のような場面では使えません。

> ではどうするかというと、static mut な変数を置いて、適当な関数から適当なタイミングで初期化することになります。とてもつらい。


`lazy-static` について
> Deref トレイトの実装に static mut な変数を持ち、初めて deref した際に渡された式で初期化する、といった感じのようです。発想が天才のそれっぽい。

from [Rust と lazy static](https://qiita.com/woxtu/items/79220899a4ebf256518c)

> lazy_static を使うと、初回アクセス時に一回だけ初期化処理が実行されるグローバル変数を作る事ができます。

from [lazy_static はもう古い!? once_cell を使おう](https://zenn.dev/frozenlib/articles/lazy_static_to_once_cell)

---

今回の場合, 問題は, `ColorCode` を初回アクセス時に初期化したいのだが, Rust はコンパイル時に初期化するため通常の関数を使えない. 

`lazy_static` クレートを使える. 

`lazy_static!` マクロで `static` を遅延初期化する (初回アクセス時に初期化される). 

```toml
# in Cargo.toml

[dependencies.lazy_static]
version = "1.0"
features = ["spin_no_std"]
```


```rust
// in src/vga_buffer.rs

use lazy_static::lazy_static;

lazy_static! {
    pub static ref WRITER: Writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };
}

```

`WRITER` は mutable がよい (immutable だと書き込みができない = `&mut self` のメソッドを使えないから) 
`static mut` は unsafe なため使いたくない. 
`RefCell` で内部可変性をもたせることもできるが, `RefCell` 型は `Sync` でない. 

### Spinlocks

1つの `WRITER` を複数の場所から呼ぶことが可能 
-> I/O mapped メモリへの write の競合が考えられる. 
-> `WRITER` への相互排他 mutual exclusion が必要.


synchronized interior mutability を得るために, 標準ライブラリでは `Mutex` が使える. `Mutex` では相互排他 mutual exclusion をスレッドをブロックすることで行う. しかし, 今の kernel には thread の概念すらないため無理. 

OS なしの基本的な mutex 機能: [spinlock](https://ja.wikipedia.org/wiki/%E3%82%B9%E3%83%94%E3%83%B3%E3%83%AD%E3%83%83%E3%82%AF)

```toml
# in Cargo.toml
[dependencies]
spin = "0.5.2"
```

```rust
// in src/vga_buffer.rs

use spin::Mutex;
...
lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}
```


---
### mutex と spinlock の相違について

ロック機構
- spinlock
- mutex
- semaphore

The Theory

スレッドが mutex をロックしようとして失敗する (すでにその mutex がロックされているため) と, そのスレッドはスリープして別のスレッドが動き出す. スレッドは起こされるまで, つまり mutex をロックしていたスレッドがアンロックするまで, スリープし続ける. 
スレッドが spinlock をロックしようとして失敗すると, スレッドは成功するまでロックしようとリトライし続ける. 

The Problem

mutex の問題は スレッドをスリープさせたり起こしたりすることが expensive operations であること (CPU 命令をたくさん必要とするし, 時間もかかる). 
mutex がロックされる時間が短い間だけであれば, スレッドをスリープさせてまた起こす時間は spinlock によるロックよりも長くかかる. 
一方で, spinlock は CPU パワーを常に消費するため, 長くロックする場合はスレッドをスリープさせたほうがよい.

The Solution

single-core/single CPU system でスピンロックを使うことは全く意味がない. 

multi-core/multi-CPU system で, かつ短い間のロックがたくさんあるような場合, mutex ロックよりも spinlock を使ったほうが効率がよくなる可能性がある. 

The practice

多くの場合, プログラマは mutex と spinlock のどちらが適しているかを事前に判別することは難しい. CPU のコア数などがわからないため.
modern OS ではハイブリッドな mutex, ハイブリッドな spinlock が使われている. 

A hybrid mutex ...

A hybrid spinlock ...

from [When should one use a spinlock instead of mutex?](https://stackoverflow.com/questions/5869825/when-should-one-use-a-spinlock-instead-of-mutex)
<!--
spinlock: 
スレッドがロックを獲得できるまで単純にループをして定期的にロックをチェックしながら待機. 
スレッドが短時間だけブロックされるなら, スピンロックは効率的. OS のプロセススケジューリングのオーバーヘッドなしにロックが可能. 
カーネル内でよく使われる. 

from [スピンロック - Wikipedia](https://ja.wikipedia.org/wiki/%E3%82%B9%E3%83%94%E3%83%B3%E3%83%AD%E3%83%83%E3%82%AF)


mutex (mutual exclusion): 
-->
---

### A println Macro

[マクロ - The Rust Programming Language 日本語版](https://doc.rust-jp.rs/book-ja/ch19-06-macros.html)

gloabl writer があるので, コードのどこからでも使える `println` マクロを追加できる. 


標準ライブラリの `println` マクロ:
```rust
#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => (print!("{}\n", format_args!($($arg)*)));
}
```

マクロは `match` と同じような arms で構成される. 
`println` マクロには 2つのルールがある. 
１つ目は引数のない呼び出し `println!()` で, `print!("\n")` に展開される. 
2 つ目は引数のある呼び出し `println!("{}, World", "Hello")` で, `print!("{}\n", format_args!(""))` に展開される. 

`#[macro_export]` でモジュール外からマクロを使用可能にする. この場合, マクロはクレートのルートに置かれる (`use std::println` となる, `use std::macros::println` ではない). 

`print` マクロ:
```rust
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::_print(format_args!($($arg)*)));
}
```


まとめ:
- spinlock/mutex
- rust のマクロ
- メモリマップド I/O


---
## spinlock と割り込み


```
main thread: ---== critical ====->
interrupt:      |  X=====>
                |  |
----------------1--2----------------
a memory segment                     
------------------------------------
```

### 問題
メインスレッドがあるメモリ区画をロックしているときに, 
割り込みが発生してそのメモリ区画をロックしようとすると, 
1. メインスレッドは割り込みのため実行されない,
2. 割り込みはメインスレッドが当該メモリ区画をロックしているためにスピンし続ける
ために, デッドロックが発生する.

### 解決方法
- spinlock でのロック時に割り込みを禁止する.
- 今回の `WRITER` であれば, 割り込み時に使われる別の writer を用意する. 

### 参考
- [Spinlocks Considered Harmful](https://matklad.github.io/2020/01/02/spinlocks-considered-harmful.html)
- [Lesson 1: Spin locks - Linux Kernel Docs](https://www.kernel.org/doc/Documentation/locking/spinlocks.txt)
- [排他制御関連 - Linuxカーネルメモ](https://wiki.bit-hive.com/linuxkernelmemo/pg/%E6%8E%92%E4%BB%96%E5%88%B6%E5%BE%A1%E9%96%A2%E9%80%A3)


