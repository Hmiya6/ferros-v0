
# Testing の一次メモ

`no_std` でのテスト. 


## Testing in Rst
標準ライブラリに依存する `test` クレートがないため, テストができない. 

### Custom Test Frameworks
`custom_test_frameworks` という unstable 機能を使って, デフォルトのテストフレームワークを置き換えることが可能. これは外部のライブラリを使わないため, `no_std` でも使用可能. 

いくつかの機能 (`should_panic` など) が利用できない. 自前で実装する必要がある. 
`#[should_panic]` では stack unwinding が使われている.

```rust
// in src/main.rs

#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}
```

`no_main` で独自のエントリーポイントを使っているため `#![test_runner(crate::test_runner)]` は無視される. 

`reexport_test_harness_main` で, 独自のテストエントリーポイントを使う. 

```rust
// in src/main.rs

#![reexport_test_harness_main = "test_main"] // `test_main` というテストエントリーポイント

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main(); // テストエントリーポイント -> `test_runner` が実行される

    loop {}
}

#[test_case]
fn trivial_assertion() {
    print!("trivial assertion... ");
    assert_eq!(1, 1);
    println!("[ok]");
}
```

## Exiting QEMU

`cargo test` ごとに手動で QEMU を終了する必要がある. 
OS を自動的にシャットダウンしたい. 

これには APM または ACPI という power management standard のサポートが必要. QEMU は `isa-debug-exit` という特別なデバイスをサポートしている. 

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = ["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
```

`bootimage runner` が テストの場合にのみ追加する引数を指定可能. 

`isa-debug-exit` というデバイス名と共に, デバイスがカーネルに到達するための *I/O port* を指定する `iobase`, `iosize` を渡す必要がある. 

### I/O Ports

x86 で CPU と peripheral hardware との間で通信する communicate 方法は 2つある: 
それが **memory-mapped I/O** と **port-mapped I/O**. 

VGA テキストバッファを使うためにメモリアドレス `0xb8000` からアクセスしたのは memory-mapped I/O. 仮想メモリアドレス `0xb8000` は RAM をマップしているのではなく VGA デバイスのメモリの一部をマップしている. 

port-mapped I/O は I/O バスを communicate のために使う. I/O port と communicate するため `in`/`out` という特別な CPU 命令がある. 

`isa-debug-exit` は port-mapped I/O を使う. `iobase` はどの port address 上で live するかを指定する (`0xf4` は x86 の IO bus として使われる). `iosize` は port サイズを指定する. 


---
追記:
## memory-mapped I/O と port-mapped I/O と CPU のアドレス空間
memory-mapped I/O と port-mapped I/O について. 
\+ その前提となる CPU のアドレス空間について. 

### CPU のアドレス空間
- CPU にはアドレスを指定するピンが生えている (AD, A ピン). 
- そのピンの数で一度に指定できるアドレスの最大数が決まる. 
- Intel 8086 の場合は AD, A ピンが計 20本ある. そのためこのプロセッサで指定できるアドレスの数 (アドレス空間) は 2^20 = 1M. 
- AD ピンはアーキテクチャの bit数存在する (AD ピンはアドレスだけでなくデータも通信するため, アーキテクチャの bit と同じ数になる. 8086 の場合は 16bit なので 16本 (+ A ピン 4本)). 

[8086 のピン](https://en.wikipedia.org/wiki/File:Intel_8086_pinout.svg)
### port-mapped I/O 
- メモリとポートでアドレス空間を別に扱うので, 狭いアドレス空間でより有効 (M/IO ピンでメモリとポートを区別). 16bit だとアドレス空間が小さい
- 現在は 2^64+ のアドレス空間を使用できるため, memory-mapped I/O で代替可能. 

### memory-mapped I/O 
- メモリと同等に扱える.
- 現在は memory-mapped I/O でもアドレス空間が足りなくなることはなくなった.

---

### Using the Exit Device

`isa-debug-exit` はとても単純. 
`value` が `iobase` で指定された IO port に書き込まれると, それは QEMU を `(value << 1) | 1` の exit status で exit する. `value` が `0` の場合は `(0 << 1) | 1 = 1`, `1` の場合は `(1 << 1) | 1 = 3` で exit する. 

`in` / `out` を手動で呼び出すのではなく, `x86_64` クレートで提供される抽象を使う. 

```toml
# in Cargo.toml

[dependencies]
x86_64 = "0.14.2"
```

```rust
// in src/main.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)] // 4-byte length
pub enum QemuExitCode {
    Success = 0x10, // (0 << 1) | 1 <- 意味がわからない. 
    Failed = 0x11, // (1 << 1) | 1 <- ここも. 
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
```

成功であれば `0x10`, 失敗であれば `0x11` を使っている. 
QEMU のデフォルトの exit code と衝突しない限り, 実際の exit code はあまり重要でない. 
例えば, exit code `0` を success のために使うと `(0 << 1) | 1 = 1` となり, 失敗コードと同じになる. 
この場合, 成功と失敗を区別することができない. 

### Success Exit Code
成功コードを指定する. 
```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = […]
test-success-exit-code = 33         # (0x10 << 1) | 1
```

## Printing to the Console

データを送信する単純な方法は serial port を使うこと. これは古いインターフェース標準でモダンなコンピュータにはない. 
serial port を使うとプログラムと QEMU はバイトをリダイレクトしてホストの標準出力やファイルに送信することができる. 

シリアルインターフェース serial interface を実装しているチップは UARTs と呼ばれる. x86 にも多くの UARTs 実装があるが, 基本的な機能は 16550 UART 互換である. 

---
### UART について
[UARTとは](https://wa3.i-3-i.info/word12982.html)
> 「信号の通り道が1つしかない通信（シリアル通信）用の信号」と「信号の通り道が複数ある通信（パラレル通信）用の信号」の変換をする...

<!-- なんのためにあるのか? コンピュータ内部ではパラレル通信だから? -->


追記: 

UART は, シリアル方式の通信規格の一つ. 
UART は単純な規格のため古いコンピュータや小さいコンピュータにも搭載されているが, 非常に遅い (9.6Kbps). 遅いため, 大きなコンピュータでは USB に置き換わった (?). 

USB (Universal Serial Bass) もシリアル方式の通信規格のひとつ. 

---

```toml
# in Cargo.toml

[dependencies]
uart_16550 = "0.2.0"
```

```rust
// in src/main.rs

mod serial;
```

```rust
// in src/serial.rs

use uart_16550::SerialPort;
use spin::Mutex;
use lazy_static::lazy_static;

// vga text buffer と同様に lazy_static, spin::Mutex を使う. 
lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = unsafe { SerialPort::new(0x3F8) }; // port を指定. 
        serial_port.init(); // port の初期化
        Mutex::new(serial_port) // Mutex による書き込み時保護
    };
}
```

簡単に使えるよう, `serial_print!`, `serial_println` マクロをつくる: 
```rust
// in src/serial.rs

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL1.lock().write_fmt(args).expect("Printing to serial failed");
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}
```

`SerialPort` には `fmt::Write` が実装してある. 


### Print an Error Message on Panic

```rust
// in src/main.rs

// our existing panic handler
// 通常の実行/コンパイル時にコンパイルされる `panic_handler`
#[cfg(not(test))] // test では not compiled.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

// our panic handler in test mode
// テストの場合のみにコンパイルされる `panic_handler`
// テストでは, serial port を通して stdio に出力される (UART と QEMU の機能を使う)
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}
```

## Hiding QEMU

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = [
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", # 自動で QEMU を終了させるため
    "-serial", "stdio", # serial を stdio で扱う. -> test のデバッグ情報を stdio に出す.
    "-display", "none" # バックグラウンドで実行される. 
]
```

`-display none` によって GUI がない環境でも実行可能になる. 

### Timeouts

`cargo test` は個々の test が return しない場合, テストが永遠に終了しない場合もある. 

他にも様々な理由で無限ループに陥る可能性がある.
- ブートローダがカーネルのロードに失敗し, 永遠に再起動し続ける.
- BIOS/UEFI ファームウェアがブートローダのロードに失敗する場合.
- QEMU の exit devie が適切に働かず, CPU が `loop {}` に入る場合.
- ハードウェアが system reset を起こす場合 (e.g. CPU 例外がキャッチされない場合).

`bootimage` はテスト実行で 5分のタイムアウトをデフォルトで行う. 

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-timeout = 300          # (in seconds)
```

### Insert Printing Automatically

`trivial_assertion` テストは `serial_print!`/`serial_println!` を使って手動でその状況を print する必要がある. 

現状:
```rust
#[test_case]
fn trivial_assertion() {
    serial_print!("trivial assertion... ");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}
```

`Testable` trait を実装する.
```rust
// in src/main.rs

// `Fn()` のラッパー `Testable` をつくる
pub trait Testable {
    fn run(&self) -> ();
}

// `Fn()` = すべての関数に `Testable` を実装する.
impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>()); // `any::type_name` は関数では関数名.
        self(); // `Fn()` 本体の実行
        serial_println!("[ok]");
    }
}

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]) { // 
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run(); // 実行
    }
    exit_qemu(QemuExitCode::Success);
}
```

## Testing the VGA Buffer
コードを書くだけなので省略

## Integration Tests
Rust の integration tests は `tests` ディレクトリに置かれる. 

`tests/` 以下の integration tests は `main.rs` とは別の実行ファイル. 
そのため, エントリーポイント等を自分で設定する必要がある. 

### Create a Library

関数を integration test で利用可能にするためには, ライブラリを `main.rs` から分離する必要がある. 

使いたいコードを `lib.rs` に置くことで, `tests` から参照できるようにできる. 

[テストの体系化](https://doc.rust-jp.rs/book-ja/ch11-03-test-organization.html) を参照. 

### Future Tests

統合テストは 別の実行ファイルとして扱われることに意味がある. 
コードが正しく CPU やハードウェアデバイスと interact しているかテストできる. 

例:
- CPU 例外: コードが不正な命令 (e.g. ゼロ除算) を実行したとき, CPU は例外を投げる. カーネルはその例外に対して handler 関数を登録できる. 統合テストは正しい例外ハンドラが呼ばれているか, また実行が正しく続くかを verify できる. 
- ページテーブル: `_start` 関数でページテーブルを改変してその効果を verify できる. 
- ユーザースペースプログラム: ユーザスペースのプログラムが禁止された操作を実行するのをカーネルが正しく防ぐかを verify できる. 

## Tests that Should Panic
コードを書くだけなので省略

通常のテストフレームワークを使わないことを明記: 
```toml
# in Cargo.toml

[[test]]
name = "should_panic"
harness = false
```
    

おわり



