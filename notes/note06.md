[Double Faults](https://os.phil-opp.com/double-fault-exceptions/)

# Double Faults のメモ


## What is a Double Fault?

単純に言えば: 
double fault は **CPU が例外ハンドラを呼び出すことに失敗したときに発生する特別の例外**

double fault は通常の例外と同様に振る舞う. ベクター `8` として IDT で通常のハンドラ関数を IDT で定義可能. double fault が失敗すれば, 致命的な triple fault が発生する. triple fault はシステムでキャッチしてハンドルすることができず, ハードウェアがシステムのリセットをかける. 

## A Double Fault Handler

double fault のハンドラ関数を追加する. 

`src/interrupts.rs`:
```rust
lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.double_fault.set_handler_fn(double_fault_handler); // new
        idt
    };
}

// new
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame, _error_code: u64) -> !
{
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}
```

## Causes of Double Faults

> double fault は **CPU が例外ハンドラを呼び出すことを失敗したときに発生する特別の例外**

「呼び出すことを失敗」とはなにか? ハンドラが存在しないことか? ハンドラがスワップアウトされたことか? ハンドラ自体が例外を発生させたのか?

AMD64 マニュアルにある定義:
> double fault *can* occur when a second exception occurs during the handling of a prior (first) exception handler. 

> double fault は先行の例外ハンドラがハンドリング中に 2つ目の例外が起こったときに発生*しうる*. 

「しうる」が重要で, 特定の例外の組み合わせのみが double fault となる. 


```
1 -> 1 か 2 -> 2 の場合は double fault が発生

---------------------------
# First Exception
## 1
- Divide-by-zero
- Invalid TSS
- Segment Not Present
- Stack-Segment Fault
- General Protection Fault
## 2
- Page Fault
---------------------------
# Second Exception
## 1
- Invalid TSS
- Segment Not Present
- Stack-Segment Fault
- General Protection Fault
## 2
- Page Fault
- Invalid TSS
- Segment Not Present
- Stack-Segment Fault
- General Protection Fault
----------------------------
```

例)
- breakpoint -> page fault: **page fault** 例外
- page fault -> page fault: **double fault** 例外
- divide-by-zero -> breakpoint: **breakpoint** 例外
- divide-by-zero -> breakpoint -> page fault: **page fault** 例外

 例外が発生すると, CPU は対応する IDT エントリーを読み出そうとする. 
- エントリーが 0 (= 無効なエントリー) であったり, general protection fault が起こる. 
- general protection fault のハンドラ関数が実装されていなければ, double fault が発生する. 


### Kernel Stack Overflow

> What happens if our kernel overflows its stack and the guard page is hit?

> カーネルがスタックをオーバーフローして guard page に被害が及んだ場合どうなるのか

guard page はスタックの一番下に特別なメモリページでスタックオーバーフローを検知するもの. 
guard page は物理フレームにマップされておらず, なのでそこにアクセスすると page fault が発生する. 
(カーネルスタックの guard page のセットアップは bootloader が行う)

page fault が発生すると CPU が page fault ハンドラを IDT を探して割り込みスタックフレームをスタックに push する. 
しかし現在のスタックポインタは存在していない guard page を指している. 
したがって 2度目の page fault が発生し, double fault が発生する. 

今度は CPU が double fault ハンドラを呼ぼうとするが, やはりスタックポインタが存在しないので, triple fault される. 

実際に triple fault を発生させる (`src/main.rs`):
```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init();
    
    // 無限に再帰させて kernel stack overflow を引き起こす
    fn stack_overflow() {
        stack_overflow();
    }
    stack_overflow();

    // * snip *
}

```

triple fault では CPU による再起動が行われるが, またオーバーフローで triple fault するので boot-loop に陥る. 

### Switching Stacks

x86_64 アーキテクチャでは例外発生時に予め定義された (predefined) 壊れていないことがわかっている (known-good) スタックに切り替えることが可能. 
この切り替えはハードウェアレベルで起こるため, CPU が exception stack frame を push する前に実行されうる. 

この切り替え機構は Interrupt Stack Table (IST) 割り込みスタックテーブルに実装されている. 
IST は 7つの known-good スタックへのポインタから成るテーブル (表) である. 

Rust 風に書くと: 
```rust
strcut InterruptStackFrame {
    stack_pointers: [Option<StackPointer>; 7];
}
```

各々のハンドラには対応する IDT エントリにある `stack_pointers` から IST のスタックを選択可能. 
例えば, double fault ハンドラには IST にある第1 のスタックを使用可能. 
CPU は double fault が発生するといつでも自動でスタックを切り替えようとする. 

QUESTION: CPU が自動で安全なスタックへ切り替えるから, 無限に page fault が起こることが避けられるということ?

## The IST and TSS

Interrupt Stack Table は Task State Segment (TSS) というレガシーな構造体の一部. TSS は 32-bit モードのタスクのための様々な情報を保持していた. 例えば hardware context switching のために使われた. しかし hardware context switching は 64-bit モードではサポートされておらず, TSS のフォーマットは完全に変更された. 

x86_64 では TSS は特定のタスクの情報を保持するものではなく, 2つの stack table (IST ともう一つ) を保持する. 32-bit と 64-bit の TSS で共通のフィールドは I/O port permissions bitmap へのポインタのみ. 

64-bit TSS のフォーマット:
```
(reserved) u32
Privilege Stack Table [u64; 3]
(reserved) u32
Interrupt Stack Table [u64; 7]
(reserved) u64
(reserved) u16
I/O Map Base Address u16
```

Privilege Stack Table は特権レベル privilege level の変更時に CPU によって使用される. 
例えば, CPU がユーザーモード (特権レベル 3) のときに例外が発生したとすると, CPU は通常, 例外ハンドラを呼び出す前にカーネルモード (特権レベル 0) に切り替わる. 
このとき, CPU は Privilege Stack Table の 0番目のスタックに切り替わる. 
(現在はユーザーモードのプログラムは存在しないため, 一旦無視)

### Creating a TSS

`src/lib.rs`:
```rust
pub mod gdt;

```

`src/gdt.rs`:
```rust
use x86_64::VirtAddr;
use x86_64::structures::tss::TaskStateSegment;
use lazy_static::lazy_static;

// double fault では IST の 0番目のスタックを用いる. 
// 別番号のスタックでも可. 
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0; 

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        // IST のセットアップ
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            // predefined, known-good stack を生成
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            
            // そのトップアドレスを TSS の IST に登録
            let stack_start = VirtAddr::from_ptr(unsafe { &STACK});
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        tss
    };
}

```

`lazy_static` を用いるのは Rust の const evaluator がコンパイル時に定数の遅延評価を扱えないため. 

まだメモリ管理を実装していないので, 新しいスタックを allocate する適切な方法がない. 代わりに, `static mut` な配列をスタックのストレージとして使用する. 
`unsafe` が必要な理由は, コンパイラは 可変定数がアクセスされたとき race freedom を保証できないから. 
`static mut` は immutable な `static` ではない, `static` は bootloader が read-only page に map される. 

QUESTION: guard page についてさらに調査が必要

### The Global Descriptor Table

Global Descriptor Table (GDT) は, ページングがデファクトスタンダードになる以前にメモリセグメンテーションのために使用されていた残余物 (relict). 
しかし, kernel/user モードの設定や TSS のローディングに使われるため, 64bit モードにおいても必要. 

GDT はプログラムのセグメント群を含む構造体. 
古いアーキテクチャでは プログラム同士を隔離するために使われた (paging がスタンダードになる前). 

QUESTION: セグメンテーションについても調査が必要

セグメンテーションは 64bit モードではサポートされていないが, GDT は存在する. GDT は現在主に 2つの目的で使われている:
- kernel/user スペースの切り替え
- TSS 構造体のロード

`src/gdt.rs`:
```rust
use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor};

lazy_static! {
    static ref GDT: GlobalDescriptorTable = {
        let mut gdt = GlobalDescriptorTable::new();
        gdt.add_entry(Descriptor::kernel_code_segment()); // kernel/user モードの切り替え (?)
        gdt.add_entry(Descriptor::tss_segment(&TSS)); // TTS を登録
        gdt
    };
}

pub fn init() {
    GDT.load();
}
```

`src/lib.rs`: 
```rust
pub fn init() {
    gdt::init();
    interrupt::init_idt();
}

```

### The Final Steps

GDT セグメントの問題は セグメントと TSS レジスタが古い GDT からの値を含んでいるために まだ active でないこと. 
また, double fault の IDT entry を設定する必要がある. 

やることは以下:

1. **code segment (CS) レジスタを再読込する**: GDT を変更したので, `cs` を再読込する必要がある. これは 古い segment selector が別の GDT デスクリプタを指している可能性があるため. 
2. **TSS のロード**: TSS セレクタを含む GDT をロードしたが, CPU にその TSS を使うよう伝える必要がある. 
3. **IDT エントリの update**: TSS をロードすると, CPU は正しい interrupt stack table (IST) にアクセス可能になる. 

1, 2 を扱う. 

`src/gdt.rs`:
```rust
use x86_64::structures::gdt::SegmentSelector;

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment()); // cs レジスタの値
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS)); // tss
        (gdt, Selectors { code_selector, tss_selector })
    };
}

// 
struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::segmentation::set_cs;
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        set_cs(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);
    }
}
```
---
QUESTION: segment selector とは?

セグメントセレクタは `cs`, `ss` などが保持する値. 
オフセットのベースとなる部分. 
他にもいくつかのフラッグを保持しており, 64bit ではこのフラッグのみが使用されている (?).

- [x86_64::structures::gdt::SegmentSelector](https://docs.rs/x86_64/0.14.4/x86_64/structures/gdt/struct.SegmentSelector.html)
- [セグメントレジスタ](https://wikiwiki.jp/north2006/%E3%82%BB%E3%82%B0%E3%83%A1%E3%83%B3%E3%83%88%E3%83%AC%E3%82%B8%E3%82%B9%E3%82%BF)

---

`src/interrupts.rs`: 
```rust
use crate::gdt;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX); // IDT に IST にあるどのスタックを使うべきか伝える
        }

        idt
    };
}

```
















