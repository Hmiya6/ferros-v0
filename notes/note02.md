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

### Memory-Related Intrinsics
> "intrinsics" は確かに「本質的な」みたいな意味がありますけど, 低レイヤプログラミングのぶんやでは用語として使われます. よくある意味としては, 普通のプログラミング環境では提供されない, CPU やシステム固有の機能群のことです. 

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

QUESTION: qemu を起動したときにキーボードとマウスの操作が乗っ取られ (?), 操作不能になってしまった. ((システムのフリーズではない.) 解除方法・防ぐ方法はある?: `Ctrl+Alt+G`, stdio をモニタとして使う

---
ここからは [Booting - Writing an OS in Rust 3rd Edition](https://github.com/phil-opp/blog_os/blob/edition-3/blog/content/edition-3/posts/02-booting/index.md) に従う.

ここまで書いてきた `.cargo/config.toml` を使うとうまく行かない. これは 以下で登場する `boot` クレートのビルド設定と, カーネルのビルド設定が競合するから. 当該ファイルはコメントアウトする.

代わりに, `.cargo/config.toml` に以下を追記:
```toml
# for `bootloader` 0.10~
[alias]
kbuild = """build --target x86_64-build-target.json -Z build-std=core \
    -Z build-std-features=compiler-builtins-mem"""
```

## UEFI
Unified Extensible Firmware Interface (UEFI) にはブートローダの実装をシンプルにする便利な機能が多くある
- CPU を 64bit モードに直接初期化する (BIOS はまず 16bit モードで初期化を行う)
- disk partition と実行ファイルを認識可能で, ディスクからロードが可能 (最初の 512byte という制限が存在しない)
- 異なる CPU アーキテクチャで同じインターフェースが使える

windows 優遇だと批判されることもある

### Boot Process

- powering on と self-testing が終わると, UEFI ファームウェアは EFI system partitions と呼ばれる bootable disk partitions を探す. 当該パーティションは fatfs である必要がある. 
- EFI system partition を見つけると, `efi/boot/bootx64.efi` (x86_64 の場合) の名前のついた実行ファイルを探す. この実行ファイルは Portable Executable (PE) フォーマット (windows でよくある形式) でなければならない. 
- その実行ファイルをメモリにロードし, 実行環境 (メモリ, CPU) をセットアップした後, 実行ファイルの entry point へジャンプする.

通常 この実行ファイルは bootloader でその後に OS カーネルがロードされる.

### How we will use UEFI

UEFI interface は協力だが, ほとんどの OS は UEFI を bootloader のためだけに使う. そのほうがコントロールしやすいため. 

### The Multiboot Standard

Multiboot と呼ばれる bootloader の標準がある. bootloader と OS のインターフェースを規定している. リファレンス実装が GNU GRUB. 

(つまり, UEFI -> GRUB -> OS = Kernel)

問題も多いため, 今回は扱わない.

## Bootable Disk Image

how to create bootable disk image: 
- 最初は cargo が `bootloader` 依存のソースコードをおいた場所を見つける.
- 次にビルドコマンドの準備


### A `boot` crate
これらのステップを手動で踏むのは面倒なので自動化する. `boot` クレートをつくる.

```sh
cargo new --bin boot
```

`Cargo.toml` に追記
```toml
[workspace]
members = ["boot"] 
```

`boot` クレートはビルドで使われる. ビルドで使われるだけなので標準ライブラリも使える.

### Locating the `bootloader` Source

> The first step in creating the bootable disk image is to locate where cargo put the source code of the `bootloader` dependency.

bootable disk image をつくる最初の段階は, cargo が `bootloader` 依存のソースコードを置く場所を見つけること.  
そのため `cargo metadata` subcommand を使うことができ, その出力には  `bootloader` クレートを含むすべての依存クレートの manifest path が含まれる. 

出力は JSON 形式だが, JSON をパースするのは道を外れすぎるので `bootloader-locator` を使う. 

```rust
// boot/src/main.rs

use bootloader_locator::locate_bootloader;

pub fn main() {
    let bootloader_manifest = locate_bootloader("bootloader").unwrap();
    dbg!(bootloader_manifest);
}
```

`locate_bootloader` 関数は bootloader の依存関係の名前を引数として取り, そのため異なる名前のブートローダも使える. 

### Running the Build Command

次の段階はビルドコマンドの実行. 

```sh
cargo builder --kernel-manifest path/to/kernel/Cargo.toml \
    --kernel-binary path/to/kernel_bin
```

これを `main` から起動したい.

```rust
// in boot/src/main.rs

use std::process::Command; // new

pub fn main() {
    let bootloader_manifest = locate_bootloader("bootloader").unwrap();

    // new code below
    // `todo!()` を使うと, 未完成のコードを表現できる
    // `std::todo!()`: https://doc.rust-lang.org/std/macro.todo.html
    let kernel_binary = todo!();
    let kernel_manifest = todo!();
    let target_dir = todo!();
    let out_dir = todo!();

    // create a new build command; use the `CARGO` environment variable to
    // also support non-standard cargo versions
    // 新しいコマンドをつくる 
    // cargo の path を得るのは, `bootloader` と同じ cargo でコンパイルすることができる
    let mut build_cmd = Command::new(env!("CARGO"));

    // set the working directory
    // コマンドを `bootloader` のディレクトリで実行する
    let bootloader_dir = bootloader_manifest.parent().unwrap();
    build_cmd.current_dir(&bootloader_dir);

    // pass the arguments
    // サブコマンド builder // これは `bootloader` の builder
    build_cmd.arg("builder"); // QUESTION: なぜ使える?? -> `bootloader` ディレクトリに移動してコマンドを実行しているから
    // コマンドライン引数
    build_cmd.arg("--kernel-manifest").arg(&kernel_manifest);
    build_cmd.arg("--kernel-binary").arg(&kernel_binary);
    build_cmd.arg("--target-dir").arg(&target_dir);
    build_cmd.arg("--out-dir").arg(&out_dir);


    // run the command
    let exit_status = build_cmd.status().unwrap();
    if !exit_status.success() {
        panic!("bootloader build failed");
    }
}

```
[Environment Variables - The Cargo Book](https://doc.rust-lang.org/cargo/reference/environment-variables.html)
cargo は環境変数を読み書きする. なぜ? コードの側から cargo を扱えるようにするため?

最終的に以下のようになった:
```rust
use bootloader_locator::locate_bootloader;
use std::process::Command;
use std::path::Path;

pub fn main() {
    let bootloader_manifest = locate_bootloader("bootloader").unwrap();
    
    // TODO: don't hardcode this
    let kernel_binary = Path::new("target/x86_64-build-target/debug/ferros")
        .canonicalize().unwrap();

    // the path to the root of this crate, set by cargo
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR")); // `boot` crate dir
    let kernel_dir = manifest_dir.parent().unwrap(); // kernel dir (`ferros` dir)
    let kernel_manifest = kernel_dir.join("Cargo.toml"); // ferros' manifest
    let target_dir = kernel_dir.join("target"); // `ferros/target`
    let out_dir = kernel_binary.parent().unwrap(); // `ferros/target/x86_64-build-target/debug`

    // create a new build command; use the `CARGO` environment variable to 
    // also support non-standard cargo versions
    let mut build_cmd = Command::new(env!("CARGO"));
    println!("{}", env!("CARGO"));

    // pass the arguments
    build_cmd.arg("builder");
    build_cmd.arg("--kernel-manifest").arg(&kernel_manifest);
    build_cmd.arg("--kernel-binary").arg(&kernel_binary);
    build_cmd.arg("--target-dir").arg(&target_dir);
    build_cmd.arg("--out-dir").arg(&out_dir);

    // set the working directory
    let bootloader_dir = bootloader_manifest.parent().unwrap();
    build_cmd.current_dir(&bootloader_dir);

    // run the command
    let exit_status = build_cmd.status().unwrap();
    if !exit_status.success() {
        panic!("bootloader build failed");
    }
}

```

諸々のツールチェーン設定 (`rust-toolchain`):
```toml
[toolchain]
channel = "nightly"
components = ["rust-src", "rustfmt", "clippy", "llvm-tools-preview"]
```
`channel` の指定や, `components` のインストールはコマンドからも可能だが, ファイルに明記することで管理がしやすくなる.

```sh
cargo kbuild
cargo run --package boot
```


BIOS として起動
```sh
qemu-system-x86_64 -drive \
    format=raw,file=target/x86_64-blog_os/debug/bootimage-bios-blog_os.img
```

UEFI として起動:
```sh
# まず, `OVMF_pure-efi.fd` をダウンロードする.
qemu-system-x86_64 -drive \
    format=raw,file=target/x86_64-blog_os/debug/bootimage-uefi-blog_os.img \
    -bios /path/to/OVMF_pure-efi.fd,
```


### やっぱり `bootloader` 0.9 を使う

VGA text buffer を使うには `bootloader` 0.9 がいい.

コマンド:
```sh
qemu-system-x86_64 -drive format=raw,file=target/x86_64-build-target/debug/bootimage-ferros.bin -monitor stdio
```



## 感想
- Rust の機能が知ることができて面白い 
- MikanOS で UEFI を使うので, BIOS でのブートではなく UEFI で進めたい -> `bootloader` 0.10~ が必要
- `bootloader` ~0.9 と 0.10~ では大きく違う. 
- Writing an OS in Rust の 3rd edition があった (2章までしかない). 
- 
- 


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









