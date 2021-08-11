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
    use x86_64::instructions::interrupts;       // new

    interrupts::without_interrupts(|| {         // new
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









