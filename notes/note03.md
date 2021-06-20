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

                self.buffer.chars[row][col].write(ScreenChar { // `write`
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





