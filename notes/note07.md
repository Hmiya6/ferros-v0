[Hardware Interrupts](https://os.phil-opp.com/hardware-interrupts/)

# Hardware Interrupts のメモ


## Overview

割り込みは取り付けられたハードウェア機器から CPU への通知を行う方法を提供する. 
つまり, カーネルに定期的にキーボードを確認する (すなわちポーリング polling) のではなく, キーボードがカーネルにキー入力を知らせることが可能. 
この方法は何かが起こった場合にのみカーネルが動くので, より効率的. 
かつ, この方法のほうがより反応速度が速い (次のポーリングを待たずに反応するため). 

すべてのハードウェア機器を CPU へ直接接続することは不可能. 
代わりに, interrupt controller が割り込みを集計 (aggregate) して CPU へ通達する. 

```
Keyboard --> | Interrupt Controller | --> | CPU
Mouse -----> |                      |     |
```

ほとんどの interrupt controllers はプログラム可能, つまり割り込みの様々な優先順位をサポートする. 
例えば, タイマーの割り込みはキーボードのそれより優先順位が高く設定されていおり, 時間の正確性を保証しようとしている. 

例外と違って, ハードウェア割り込みは非同期 asynchronously に発生する. 
ゆえに, 並行処理関連のバグが課題となる. 

## The 8259 PIC

Intel 8259 は 1976年に導入された programmable interrupt controller (PIC). 
より新しい APIC に置き換わって久しいが, 現在のシステムでも後方互換性のためにサポートが続いている. 
8259 PIC は APIC よりもセットアップが極めて簡単なので, まずは前者を使う. 

8259 には 8つの interrupt lines があり, CPU と通信するための複数の線 line も存在する. 
典型的なシステムには 2つの 8259 PIC が備わっており, 以下のように接続されている. 


```
                     ____________                          ____________
Real Time Clock --> |            |   Timer -------------> |            |
ACPI -------------> |            |   Keyboard-----------> |            |      _____
Available --------> | Secondary  |----------------------> | Primary    |     |     |
Available --------> | Interrupt  |   Serial Port 2 -----> | Interrupt  |---> | CPU |
Mouse ------------> | Controller |   Serial Port 1 -----> | Controller |     |_____|
Co-Processor -----> |            |   Parallel Port 2/3 -> |            |
Primary ATA ------> |            |   Floppy disk -------> |            |
Secondary ATA ----> |____________|   Parallel Port 1----> |____________|
```

> 8259はマルチプレクサ、つまり一つのデバイスに割り込みをかけるため、複数の割り込み入力を一つの割り込み出力に束ねるように振舞う。

[Intel 8259 - Wikipedia](https://ja.wikipedia.org/wiki/Intel_8259) より

それぞれのコントローラは 2つの I/O ポートを通して設定される. 
一つは "command" ポートで, もう一つは "data" ポート. 
primary controller に対しては `0x20` (command), `0x21` (data) のポートが割り当てられている. 
secondary controller には `0xa0` (command), `0xa1` (data). 

### Implementation

PIC はデフォルト設定では 0-15 の割り込みベクタを CPU へ送信するため, 設定を変更する必要がある 
(他の割り込みベクタとかぶってしまう. 例えば, CPU 例外の double fault の割り込みベクタは 8). 
これを解決するため, PIC 割り込みを別の番号に remap することが必要. 
番号に指定はないが, 典型的には 32-47 が選ばれる (例外のために割り当てられた 1-32 のスロットの後). 

今回は `pic8259` クレートを使う. 

`Cargo.toml`: 
```toml
[dependencies]
# * snip *
pic8259 = "0.10.1"
```

`ChainedPics` を使うことで, 上記の primary/secondary PIC のレイアウトを使用可能. 

`src/interrupts.rs`: 
```rust
use pic8259::ChainedPics;
use spin; // spin lock を用いた mutex の実装 (詳しくは `notes/note03.md`)

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

// `lazy_static` ではない
pub static PICS: spin::Mutex<ChainedPics> = spin::Mutex::new(
    unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) }
);
```

`src/lib.rs`: 
```rust
pub fn init() {
    gdt::init(); 
    interrupts::init_idt(); 
    unsafe { interrupts::PICS.lock().initialize() }; // `initialize` で PIC を初期化
}
```

## Enabling Interrupts

割り込みは, CPU 設定で無効化されている. 
つまり CPU は interrupt controller からの割り込みを受け付けない. 

`src/lib.rs`: 
```rust
pub fn init() {
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable(); // `sti` ("set interrupts") 命令で外部割り込みを有効化. 
}
```

Timer による外部割り込みで, ハンドラの実装していないので double fault が起こってしまう. 

## Handling Timer Interrupts

上の図にあるとおり, timer は primary PIC の 0番目の線を使用している. 
つまり CPU は 32番割り込み (32 + 0) .

`src/interrupts.rs`: 
```rust
#[derive(Debug, Clone, Copy)]
#[repr(u8)] // それぞれの変数は `u8` で表現される
// ハードウェアからの割り込み番号を保存する enum
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    // あとで追加
}

// index として使えるように
impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}
```

`src/interrupts.rs`: 
```rust
use crate::print;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        […]
        // timer のためのハンドラ関数を登録
        idt[InterruptIndex::Timer.as_usize()] // 上で作った `InterruptIndex` で index を指定する. 
            .set_handler_fn(timer_interrupt_handler);

        idt
    };
}

// ハンドラ関数 (割り込みのため, 特殊な呼出規約 `x86-interrupt` が必要.)
extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    print!(".");

    unsafe {
        // 割り込みの終了を PIC に伝える. 
        // これによって PIC が次の割り込みを行えるようになる. 
        // 
        // "end of interrupt" (EOI) シグナルを PIC へ送信.
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}
```


## Deadlocks
カーネルに 並行処理の要素ができた. 
つまり, タイマー割り込みは非同期に発生し, `_start` 関数にいつでも割り込むことができる. 
幸運なことに, Rust の所有権システムは並行処理に関するバグの多くを防ぐ. 
特筆すべき例外は **デッドロック (deadlock)**. 
デッドロックはスレッドが free されないロックを得ようとして発生する. 
この場合, スレッドは 未定義にハングする. 

デッドロックの例: 
```
1. `println!` を呼び出す
2. `print` 関数が `WRITER` をロックする
3. **割り込みが発生し**, ハンドラが走る
4. ハンドラの `print` 関数が (すでにロックされている) `WRITER` をロックしようとする
... loop ...
```

`WRITER` はロックされているので, 割り込みハンドラは `WRITER` が free されるまで待ち続ける. 
しかし, free されることはない. 
それは `_start` 関数が割り込みハンドラが return した後も走り続けるから. 

### Provoking a Deadlock
試すだけ

### Fixing the Deadlock
このデッドロックを回避するためには, `Mutex` は lock されている間, 割り込みを無効にする. 

`src/vga_buffer.rs`: 
```rust
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;   // `without_interrupts` を使えるようにインポート

    interrupts::without_interrupts(|| {     // (クロージャ実行の間) 割り込み禁止命令
        WRITER.lock().write_fmt(args).unwrap();
    });
}
```

`without_interrupts` 関数は closure をとって割り込みなしの環境で実行する. 

シリアル通信での print 関数にもこの変更を加える. 

`src/serial.rs`: 
```rust
#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        SERIAL1
            .lock()
            .write_fmt(args)
            .expect("Printing to serial failed");
    });
}
```

割り込みの無効化は全般的な解決 general solution にはならない (するべきでない). 
問題は割り込みの遅延が増加すること. 
なので割り込みの無効化は短い時間であるべきである. 

## Fixing a Race Condition
テストの変更. コードを書くだけ. 


## The `hlt` Instruction
ここまで `_start` や `panic` 関数で単純な空ループを使っていた. 
これは CPU を無限にスピンさせるためとても非効率. 
常に CPU 稼働率が 100% になってしまう. 

なので CPU を割り込みまで停止させたい. 
これによって CPU は休止状態に入り, 消費エネルギーを軽減可能. 
`hlt` 命令でこれを実現する. 

コード中の `loop {}` を `hlt_loop` へ変更.

## Keyboard Input

外部からの割り込みをハンドルできるようになったので, キーボード入力をサポート可能になった. 

> PS/2 キーボードを操作する. USB キーボードではない. USB キーボードは PS/2 キーボードとして emulate されるので, USB キーボードは無視する. 

ハードウェアタイマーと同様に, キーボードコントローラーはデフォルトで有効化されている. 
なのでキーを押すと キーボードコントローラーは PIC に割り込みを送る. 
CPU は IDT からハンドラ関数を探すが, 対応するエントリは空なので double fault が発生する. 

キーボード入力のためのハンドラ関数を追加する. 

`src/interrupts.rs`: 
```rust
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard, // キーボードを追加 // 何も指定しない場合, Timer + 1 の値が自動的に割り当てられる. 
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        […]
        // ハンドラ関数を追加
        idt[InterruptIndex::Keyboard.as_usize()]
            .set_handler_fn(keyboard_interrupt_handler);

        idt
    };
}

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    print!("k");

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

上の図にあるとおり, primary PIC の 1番の線を使用する. 
つまり CPU は 33番割り込み (1 + 32) として到達する. 


### Reading the Scancodes 
どの key を押したかを判断するため, キーボードコントローラを query する必要がある. 
PS/2 コントローラーの data port (= `0x60`番 I/O port) から読み取って行う. 

`src/interrupts.rs`: 
```rust
extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60); // `0x60` ポート
    let scancode: u8 = unsafe { port.read() }; // ポートの読み取りは unsafe
    print!("{}", scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

```

### Interpreting the Scancodes

コードから実際の文字への変換は (自力でも可能だが) `pc-keyboard` クレートを使って簡易的に行う. 

`Cargo.toml`: 
```toml
[dependencies]
pc-keyboard = "0.5.0"
```

`src/interrupts.rs`: 
```rust
extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use spin::Mutex;
    use x86_64::instructions::port::Port;
    
    // 初回アクセスだけ実行される
    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Jis109Key, ScancodeSet1>> =
            Mutex::new(Keyboard::new(
                layouts::Jis109Key, // JIS キーボード
                ScancodeSet1, // ?
                HandleControl::Ignore, // `Ctrl+[a-z]` を `U+0001` - `U+001A` へ
            ));
    }

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);

    let scancode: u8 = unsafe { port.read() }; // port の読み取り
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) { // `Option<KeyEvent>` に変換. 
        if let Some(key) = keyboard.process_keyevent(key_event) { // キーイベントを文字へ変換. 
            match key {
                DecodedKey::Unicode(character) => print!("{}", character), // unicode 文字の場合
                DecodedKey::RawKey(key) => print!("{:?}", key), // 特殊文字の場合
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```


## spinlock まとめ

ロックについて: 
> ロックは排他制御の方式の一つで、コンピュータ内で並行して実行されているプログラム（スレッドやプロセス）のうち、ある一つが特定の資源（メモリ領域）を利用し始めると、処理が終わるまで他の主体によるアクセスを禁じる仕組みである。

ロック以外の排他制御方式を知らない. 

スピンロックについて: 
> スピンロックは最も単純なロック方式の一つで、利用したい資源がロックされて待たされている他のプログラムが、単純にロック状態をチェックするだけのループ（繰り返し）処理を実行し続ける方式である。待っている間に他の処理を行うことができず処理効率は低くなるが、実装や制御が容易である。ロックの単位が小さく（ロック粒度が細かい）、一回のロック時間が短いことが見込まれるシステムに向いている。

[スピンロックとは](https://e-words.jp/w/%E3%82%B9%E3%83%94%E3%83%B3%E3%83%AD%E3%83%83%E3%82%AF.html)


`spin::Mutex` について: 
> This structure behaves a lot like a normal Mutex. There are some differences:

>   - It may be used outside the runtime.
>       - A normal mutex will fail when used without the runtime, this will just lock
>       - When the runtime is present, it will call the deschedule function when appropriate
>   - No lock poisoning. When a fail occurs when the lock is held, no guarantees are made

> When calling rust functions from bare threads, such as C pthreads, this lock will be very helpful. In other cases however, you are encouraged to use the locks from the standard library.

[spin クレートのドキュメント](https://doc.redox-os.org/std/spin/index.html) より

## `pic8259` クレートについて

> Abstractions for 8259 and 8259A Programmable Interrupt Controller (PICs). 

キーボード (ハードウェア) と `pic8259` はどうつながっている? ギャップがある. 

-> qemu に組み込まれた 8259 エミュレータ (?) の API

### 8259 PIC について

8259 PIC は CPU の割り込みメカニズムをコントロールする. 
これは 1. 複数の割り込みリクエストを受け付け, 2. それらをプロセッサへ順に feed する ことでコントロールする.  
例えば, キーボードが keyhit を register すると, その interrupt line (IRQ1) を通して PIC チップへ pulse を送る, IRQ を system interrupt へ変換し, CPU に割り込むためメッセージを送信する. 
ここでのカーネルの仕事は これらの IRQs をハンドルし, 必要な手順を実行する. 

PIC がないと, システム内のすべての機器を poll する必要がある. 


### `pic8259` クレートの実装
```rust
//! Support for the 8259 Programmable Interrupt Controller, which handles
//! basic I/O interrupts.  In multicore mode, we would apparently need to
//! replace this with an APIC interface.
//!
//! The basic idea here is that we have two PIC chips, PIC1 and PIC2, and
//! that PIC2 is slaved to interrupt 2 on PIC 1.  You can find the whole
//! story at http://wiki.osdev.org/PIC (as usual).  Basically, our
//! immensely sophisticated modern chipset is engaging in early-80s
//! cosplay, and our goal is to do the bare minimum required to get
//! reasonable interrupts.
//!
//! The most important thing we need to do here is set the base "offset"
//! for each of our two PICs, because by default, PIC1 has an offset of
//! 0x8, which means that the I/O interrupts from PIC1 will overlap
//! processor interrupts for things like "General Protection Fault".  Since
//! interrupts 0x00 through 0x1F are reserved by the processor, we move the
//! PIC1 interrupts to 0x20-0x27 and the PIC2 interrupts to 0x28-0x2F.  If
//! we wanted to write a DOS emulator, we'd presumably need to choose
//! different base interrupts, because DOS used interrupt 0x21 for system
//! calls.

#![no_std]

use x86_64::instructions::port::Port; // 外部クレートは port のみ

/// Command sent to begin PIC initialization.
const CMD_INIT: u8 = 0x11; // command port から送る init コマンド 

/// Command sent to acknowledge an interrupt.
const CMD_END_OF_INTERRUPT: u8 = 0x20; // command port から送る EOI コマンド

// The mode in which we want to run our PICs.
const MODE_8086: u8 = 0x01; // command port から送る 8086モード の命令

/// An individual PIC chip.  This is not exported, because we always access
/// it through `Pics` below.
struct Pic {
    /// The base offset to which our interrupts are mapped.

    offset: u8,

    /// The processor I/O port on which we send commands.
    command: Port<u8>, // command の送信に使う IO port 

    /// The processor I/O port on which we send and receive data.
    data: Port<u8>, // data の送受信に使う IO port
}

impl Pic {
    /// Are we in change of handling the specified interrupt?
    /// (Each PIC handles 8 interrupts.)
    fn handles_interrupt(&self, interupt_id: u8) -> bool {
        self.offset <= interupt_id && interupt_id < self.offset + 8
    }

    /// Notify us that an interrupt has been handled and that we're ready
    /// for more.
    // 
    // command port に EOI コマンドを書き込む = PIC に EOI を知らせる
    unsafe fn end_of_interrupt(&mut self) {
        self.command.write(CMD_END_OF_INTERRUPT);
    }

    /// Reads the interrupt mask of this PIC.
    unsafe fn read_mask(&mut self) -> u8 {
        self.data.read()
    }

    /// Writes the interrupt mask of this PIC.
    unsafe fn write_mask(&mut self, mask: u8) {
        self.data.write(mask)
    }
}

/// A pair of chained PIC controllers.  This is the standard setup on x86.
pub struct ChainedPics {
    pics: [Pic; 2],
}

impl ChainedPics {
    /// Create a new interface for the standard PIC1 and PIC2 controllers,
    /// specifying the desired interrupt offsets.
    pub const unsafe fn new(offset1: u8, offset2: u8) -> ChainedPics {
        ChainedPics {
            pics: [
                Pic {
                    offset: offset1,
                    command: Port::new(0x20),
                    data: Port::new(0x21),
                },
                Pic {
                    offset: offset2,
                    command: Port::new(0xA0),
                    data: Port::new(0xA1),
                },
            ],
        }
    }

    /// Initialize both our PICs.  We initialize them together, at the same
    /// time, because it's traditional to do so, and because I/O operations
    /// might not be instantaneous on older processors.
    pub unsafe fn initialize(&mut self) {
        // We need to add a delay between writes to our PICs, especially on
        // older motherboards.  But we don't necessarily have any kind of
        // timers yet, because most of them require interrupts.  Various
        // older versions of Linux and other PC operating systems have
        // worked around this by writing garbage data to port 0x80, which
        // allegedly takes long enough to make everything work on most
        // hardware.  Here, `wait` is a closure.
        let mut wait_port: Port<u8> = Port::new(0x80);
        let mut wait = || wait_port.write(0);

        // Save our original interrupt masks, because I'm too lazy to
        // figure out reasonable values.  We'll restore these when we're
        // done.
        let saved_masks = self.read_masks();

        // Tell each PIC that we're going to send it a three-byte
        // initialization sequence on its data port.
        // それぞれの PIC を初期化するコマンドを送信. 
        self.pics[0].command.write(CMD_INIT);
        wait();
        self.pics[1].command.write(CMD_INIT);
        wait();

        // Byte 1: Set up our base offsets.
        // オフセットを data port へ書き込む
        self.pics[0].data.write(self.pics[0].offset);
        wait();
        self.pics[1].data.write(self.pics[1].offset);
        wait();

        // Byte 2: Configure chaining between PIC1 and PIC2.
        // pic 同士を chain させる. 
        self.pics[0].data.write(4);
        wait();
        self.pics[1].data.write(2);
        wait();

        // Byte 3: Set our mode.
        // pic の動作モードを data port へ書き込む. 
        self.pics[0].data.write(MODE_8086);
        wait();
        self.pics[1].data.write(MODE_8086);
        wait();

        // Restore our saved masks.
        self.write_masks(saved_masks[0], saved_masks[1])
    }

    /// Reads the interrupt masks of both PICs.
    pub unsafe fn read_masks(&mut self) -> [u8; 2] {
        [self.pics[0].read_mask(), self.pics[1].read_mask()]
    }

    /// Writes the interrupt masks of both PICs.
    pub unsafe fn write_masks(&mut self, mask1: u8, mask2: u8) {
        self.pics[0].write_mask(mask1);
        self.pics[1].write_mask(mask2);
    }

    /// Disables both PICs by masking all interrupts.
    pub unsafe fn disable(&mut self) {
        self.write_masks(u8::MAX, u8::MAX)
    }

    /// Do we handle this interrupt?
    pub fn handles_interrupt(&self, interrupt_id: u8) -> bool {
        self.pics.iter().any(|p| p.handles_interrupt(interrupt_id))
    }

    /// Figure out which (if any) PICs in our chain need to know about this
    /// interrupt.  This is tricky, because all interrupts from `pics[1]`
    /// get chained through `pics[0]`.
    // 
    pub unsafe fn notify_end_of_interrupt(&mut self, interrupt_id: u8) {
        if self.handles_interrupt(interrupt_id) {
            if self.pics[1].handles_interrupt(interrupt_id) {
                self.pics[1].end_of_interrupt();
            }
            self.pics[0].end_of_interrupt();
        }
    }
}
```















