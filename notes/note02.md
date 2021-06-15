[A Minimal Kernel](https://os.phil-opp.com/minimal-rust-kernel/)
# A Minimal Kernel 一次メモ

## The Boot Process
コンピュータを起動すると, マザーボード ROM に保存されているファームウェアが実行する

ファームウェアの役割
- power-on self-test
- 利用可能な RAM の検出
- CPU とハードウェアの初期化 (pre-initialize)
最後に,
- ブート可能なディスクを探し, OS カーネルを起動する

x86 の標準的なファームウェア
- Basic Input/Output System (BIOS)
- Unified Extensible Firmware Interface (UEFI)

BIOS は時代遅れだけどシンプル. UEFI はモダンで多くの機能があるが複雑.

## A Minimal Kernel

`cargo` を使って freestanding binary をビルドしてきたが, 通常 `cargo` はホストシステムのためにビルドする. 

Rust の OS をビルドするためには nightly でしか提供されない実験的な機能が必要になる.

nightly の導入方法
1. shell でこのディレクトリで nightly の Rust を使うことを伝える.
```sh
rustup override set nightly
```
2. `rust-toolchain` に `nightly` と書く. (git で管理しててもわかりやすいため, これを採用)


`cargo` はコンパイルのバックエンドに LLVM を使っている. そのため, target triple でコンパイル先を変更可能 (参考: [target triple](https://clang.llvm.org/docs/CrossCompilation.html#target-triple)).

通常のバイナリで下層にあるはずの OS が, 今回は存在しない. なので, 既存の target triple は使えない. 自分で設定する必要がある.


通常の `x86_64-unknown-linux-gnu` を指定する場合の JSON.
```json
{
    "llvm-target": "x86_64-unknown-linux-gnu",
    "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "linux",
    "executables": true,
    "linker-flavor": "gcc",
    "pre-link-args": ["-m64"],
    "morestack": false
}

```

一部分をもらって, 
```json
{
    "llvm-target": "x86_64-unknown-none",
    "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "none",
    "executable": true
}
```
メモ: json はコメントが書けない. toml で書きたい.

---
疑問点: 上の json に何を書くべきなのか, 何を書けるのかわからない.

-> あった [target specification](https://github.com/rust-lang/rfcs/blob/master/text/0131-target-specification.md)

---

`"data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",`:  
どのようにメモリにデータを置くかを指定. 詳しいことは [Data Layout](https://llvm.org/docs/LangRef.html#data-layout).

```json
"linker-flavor": "ld.lld",
"linker": "rust-lld",
```
デフォルトのリンカではなく, LLD を使う. (Linux ターゲットに対応していない可能性がある(?))

```json
"panic-strategy": "abort",
```
stack unwinding の代わりにどう処理を行うかを指定. `Cargo.toml` に `panic = "abort"` と書くのと同義. (ただ, `Cargo.toml` のオプションと異なり, ここで指定する場合は `core` ライブラリを再コンパイルする場合も適用される.)

TODO: `core` ライブラリについて調べる

```json
"disable-redzone": true,
```

カーネルを書いているため, どこかで interrupt をハンドルする必要がある. それを安全におこなうために "red zone" と呼ばれるスタックポインタの最適化を無効にする.  

TODO: red zone について調べる [disabling the red zone](https://os.phil-opp.com/red-zone/)

```json
"features": "-mmx,-sse,+soft-float",
```
`features` は target features を有効化/無効化する.  
ここでは `mmx`, `sse` を無効化し, `soft-float` を有効化する.

`mmx` と `sse` は Single Instruction Multiple Data (SIMD) 命令群をサポートするため機能 (有効化すると速くなる). 

(`x86_64` では浮動小数点演算に SIMD レジスタがデフォルトで必要. それを解決するため, `soft-float` で SIMD なしで演算をするように指定している. )

TODO: SIMD について調べる [disabling SIMD](https://os.phil-opp.com/disable-simd/)



## Building our Kernel

> Compiling for our new target will use Linux conventions (I'm not quite sure why, I assume that it's just LLVM's default). This means that we need an entry point named `_start` as described in the previous post.
新しいターゲットへのコンパイルは Linux conventions (なぜかは確かではないが, ただ LLVM のデフォルトだから (?)) を使う. つまりエントリーポイント `_start` が必要

```
╰─λ cargo build --target x86_64-build-target.json                                101 (0.557s) < 12:59:08

error[E0463]: can't find crate for `core`
```

`core` が見つからないというエラーがでる.

[`core` ライブラリ](https://doc.rust-lang.org/nightly/core/index.html)には 基本的な Rust の型である `Result` や `Option` やイテレータが含まれる. `core` はすべての `no_std` クレート (= このプロジェクト) に非明示的にリンクされている.

問題は `core` ライブラリが Rust のコンパイラと共に **precompiled ライブラリ**として配布されていること. なので, ホストの target triple (= この PC では`x86_64-unknown-linux-gnu`) のみで valid. なので, `core` ライブラリも再コンパイルする必要がある.

### The `build-std` Option

そんなときのための機能が `build-std`. これによって `core` とその他の標準ライブラリクレートを必要であれば再コンパイルすることが可能 (新しい機能なので unstable とされている. nighyly のみで使用可能).

この機能を使うには `.cargo/config.toml` をいじる必要がある.
```toml
[unstable]
build-std = ["core" ,"compiler_builtins"]
```
また, `rustup component add rust-src` も `core` の再コンパイルのため必要.

MEMO: `build-std` を (プロジェクトごとに) 切り替えで使えるようしたい.

### Memory-Related Intrinsics (メモリ関連の本質的なことについて)
MEMO: intrinsic - 本来備わっている, 固有の, 本質的な
TODO: C (とくにメモリの部分) についての理解が必要.

Rust コンパイラはすべてのシステムでいくつかの built-in 関数群が使用可能であることを仮定している. `compiler_builtins` で多くが提供される. しかし, いくつかのメモリ関連の関数群はデフォルトで有効になっていない. 通常はシステム側の C ライブラリで提供されるからである. これには `memset`, `memcpy` ,`memcmp` が含まれる. (このカーネルをコンパイルするのにすぐ必要になるわけではないが, そのうち必要になる)

OS の C ライブラリにリンクできないため, 上記の関数をコンパイラに与える別の方法が必要. `#[no_mangle]` を使って `memset` などを自力で実装する方法もあるが, これは非常に危険. なので既存の well-tested な実装を利用するのがよい.  
(`memcpy` の自力実装の例: `memcpy` を `for` ループで実装しようとすると, `for` が `IntoIterator::into_iter` を呼び出して, それが `memcpy` を再び呼び出したりする.)

`compiler_builtins` には上記の"既存実装"がある. これらはシステムのもの (C ライブラリ) と競合しないように無効化されているだけ. 

`.carogo/config.toml` に以下を追加して有効化:
```toml
[unstable]
build-std-features = ["compiler-builtins-mem"]
build-std = ["core", "compiler_builtins"]
```

これで コンパイラに必要な関数が全て揃った.

### Set a Default Target
`.cargo/config.toml` 
```toml
[build]
target = "target.json"
```

MEMO: `.cargo/config.toml` はプロジェクトごとで OK. [Configuration - The Cargo Book](https://doc.rust-lang.org/cargo/reference/config.html)



## Printing to Screen

この段階で画面にテキストをプリントする最も簡単な方法は [VGA text buffer](https://en.wikipedia.org/wiki/VGA-compatible_text_mode). VGA については後で.


```rust
static HELLO: &[u8] = b"Hello World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // 可変な生ポインタ (`*mut` は可変な生ポインタを示す型)
    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte; // 文字の byte
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb; // 色の byte (0xb = light cyan)
        }
    }

    loop {}
}
```
VGA buffer で文字をプリントするには `0xb8000` のアドレスにバッファを置く.

`unsafe` で生ポインタの参照外しを行っている.

---
### Rust の生ポインタ
生ポインタ自体は safe でも生成可能.  
(safe では生ポインタの参照外し (= 値の read) ができない).
- 借用規則を無視できる
- 指している先のメモリが有効な値を持つか保証されない
- null の可能性がある
- 自動的な cleanup は実装されていない

例: ある値の参照から生ポインタを生成する
```rust
let mut num = 5;

// num の不変参照 (&num)・可変参照 (&mut num) の生ポインタ
let r1 = &num as *const i32;
let r2 = &mut num as *mut i32;
```
例: 任意のメモリアドレスへの生ポインタを生成する
```rust
let address = 0xb8000;
let r = address as *const i32;

```
[生ポインタを参照外しする](https://doc.rust-jp.rs/book-ja/ch19-01-unsafe-rust.html#%E7%94%9F%E3%83%9D%E3%82%A4%E3%83%B3%E3%82%BF%E3%82%92%E5%8F%82%E7%85%A7%E5%A4%96%E3%81%97%E3%81%99%E3%82%8B)

---

## Running our Kernel

カーネルを bootable disk image にするにはカーネルを bootloader にリンクする必要がある. bootloader が CPU と他のハードウェアを初期化し, カーネルをロードする.

`bootloader` クレートを使う: [bootloader](https://crates.io/crates/bootloader)

`bootloader` 0.10 ~ から, 大きな変更がある. (BIOS だけでなく **UEFI に対応**. 使い方も変更.)

QUESTION: qemu を起動したときにキーボードとマウスの操作が乗っ取られ (?), 操作不能になってしまった. ((システムのフリーズではない.) 解除方法・防ぐ方法はある?

ここからは [Booting - Writing an OS in Rust 3rd Edition](https://github.com/phil-opp/blog_os/blob/edition-3/blog/content/edition-3/posts/02-booting/index.md) に従う.

<!--
### 補足: LLVM
LLVM についてほとんど知らないことがわかったので補足

参考
- [LLVM](https://llvm.org/doxygen/index.html)
- [こわくないLLVM入門!](https://qiita.com/Anko_9801/items/df4475fecbddd0d91ccc)
- [大学院生のためのLLVM](https://postd.cc/llvm-for-grad-students/)
- [LLVM for Grad Students](https://www.cs.cornell.edu/~asampson/blog/llvm.html)


> コンパイラは通常フロントエンド、ミドルエンド、バックエンドに分けられ、各プロセスで様々な処理をしています。特にミドルエンド、バックエンドでは中間言語や各アーキテクチャに対するたくさんの最適化を施さなければなりません。この最適化を預けてフロントエンドだけを考えればコンパイラが作れるというものがLLVMです。LLVMは強力な型システムや厳密な制約を持っていて、これにより高度な最適化技術は実現します。更にLLVMはJITを作る事ができます。JITは通常実装するのが大変ですが、LLVMを使えば楽に実装できます。更に更に他の言語でコンパイルされたLLVMの言語とリンクする事ができます。だから自分で作った言語でC言語の関数を使うことができます！
こわくないLLVM入門 より

-->









