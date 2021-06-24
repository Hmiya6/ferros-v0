
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

### Using the Exit Device

`isa-debug-exit` はとても単純. 
`value` が `iobase` で指定された IO port に書き込まれると, それは QEMU を `(value << 1) | 1` の exit status で exit する. `value` が `0` の場合は `(0 << 1) | 1 = 1`, `1` の場合は `(1 << 1) | 1 = 3` で exit する. 

`in` / `out` を手動で呼び出すのではなく, `x86_64` クレートで提供される抽象を使う. 

```

```















