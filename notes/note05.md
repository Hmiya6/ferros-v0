[CPU Exceptions](https://os.phil-opp.com/cpu-exceptions/)

# CPU Exceptions のメモ

CPU 例外が起こる状況
- 不正なメモリアドレスへのアクセス
- ゼロ除算

これらに対応するため, **interrupt descriptor table** をセットアップしなければならない. 

## Overview

例外は実行中の命令の不良をシグナルする. 
例外が発生すると, CPU は現在の作業に割り込んで, 特定の例外ハンドラ関数を呼び出す. 

x86 には約20の CPU 例外があり, 重要なのは以下:
- **Page Fault**: 不正なメモリアクセスによって発生する. 具体的には, 命令が map されていないページから read しようとしたり, read-only ページへ書き込みを行おうとする場合.
- **Invalid Opcode**: 命令が不正な場合に発生する例外. 例えば, 古い CPU でサポートされていない新しい命令を使う場合.
- **General Protection Fault**: 様々な種類のアクセス違反の際に起こる例外. 例えば, 特権命令 privileged instruction をユーザレベルのコードで実行しようとする場合や, 設定レジスタ configuration registers の reserved field へ書き込もうとした場合. 
- **Double Fault**: 例外が起きたとき, CPU は対応したハンドラ関数を呼び出そうとする. その例外ハンドラを呼び出す間に別の例外が発生すると, CPU は double fault 例外を発する. 例外に対応したハンドラ関数が存在しない場合もこの例外が呼ばれる. 
- **Triple Fault**: double fault 例外ハンドラ関数を呼び出そうとする間に例外が起きたとき, CPU は致命的な triple fault を発する. triple fault は catch or handle できない. 多くのプロセッサでは CPU のリセットと OS の再起動を行う. 


### The Interrupt Descriptor Table
例外を catch and handle するために, **Interrupt Descriptor Table (IDT)** と呼ばれるものを構築する必要がある. 
この table には各 CPU 例外に対応するハンドラ関数を指定する. ハードウェアはこの table を直接使うため, フォーマットに従う必要がある. 

各エントリ (table の要素) のメモリ構造: 
```
Type	Name	Description
u16	Function Pointer [0:15]	The lower bits of the pointer to the handler function.
u16	GDT selector	Selector of a code segment in the global descriptor table.
u16	Options	(see below)
u16	Function Pointer [16:31]	The middle bits of the pointer to the handler function.
u32	Function Pointer [32:63]	The remaining bits of the pointer to the handler function.
u32	Reserved	

```
`Options` の詳細: 
```
Bits	Name	Description
0-2	Interrupt Stack Table Index	0: Don't switch stacks, 1-7: Switch to the n-th stack in the Interrupt Stack Table when this handler is called.
3-7	Reserved	
8	0: Interrupt Gate, 1: Trap Gate	If this bit is 0, interrupts are disabled when this handler is called.
9-11	must be one	
12	must be zero	
13‑14	Descriptor Privilege Level (DPL)	The minimal privilege level required for calling this handler.
15	Present	

```

各例外には IDT index がある. 
例えば invalid opcode 例外は 6, page fault 例外は 14. 
他の例外の IDT index については [Exception Table](https://wiki.osdev.org/Exceptions) の "Vector nr." にある. 


例外が起きたとき, CPU はだいたい以下を行う: 
1. スタックにいくつかのレジスタを push. これには命令へのポインタや RFLAG レジスタ (詳しくは後述) が含まれる. 
2. Interrupt Descriptor Table (IDT) から対応したエントリを読み出す. 例えば, page fault が起こった場合は CPU は 14番目のエントリを読み出す. 
3. 当該エントリが存在しなければ, double fault を挙げる. 
4. エントリが interrupt gate であれば (bit 40 がセットされていなければ), ハードウェア割り込みを無効化する. 
5. 指定された GDT セレクタを CS segment へロードする. 
6. 指定されたハンドラ関数へ jump する. 

## An IDT Type

`x86_64` クレートの [InterruptDescriptorTable 構造体](https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptDescriptorTable.html) を使う. 

```rust
#[repr(C)]
#[repr(align(16))]
pub struct InterruptDescriptorTable {

    pub divide_error: Entry<HandlerFunc>,
    pub debug: Entry<HandlerFunc>,
    pub non_maskable_interrupt: Entry<HandlerFunc>,
    pub breakpoint: Entry<HandlerFunc>,
    pub overflow: Entry<HandlerFunc>,
    pub bound_range_exceeded: Entry<HandlerFunc>,
    pub invalid_opcode: Entry<HandlerFunc>,
    pub device_not_available: Entry<HandlerFunc>,
    pub double_fault: Entry<DivergingHandlerFuncWithErrCode>,
    pub invalid_tss: Entry<HandlerFuncWithErrCode>,
    pub segment_not_present: Entry<HandlerFuncWithErrCode>,
    pub stack_segment_fault: Entry<HandlerFuncWithErrCode>,
    pub general_protection_fault: Entry<HandlerFuncWithErrCode>,
    pub page_fault: Entry<PageFaultHandlerFunc>,
    pub x87_floating_point: Entry<HandlerFunc>,
    pub alignment_check: Entry<HandlerFuncWithErrCode>,
    pub machine_check: Entry<DivergingHandlerFunc>,
    pub simd_floating_point: Entry<HandlerFunc>,
    pub virtualization: Entry<HandlerFunc>,
    pub security_exception: Entry<HandlerFuncWithErrCode>,
    // some fields omitted
    // 256 entries in total
}
```

`idt::Entry<F>` は IDT entry を表す構造体. `F` はハンドラ関数の型.
- `HandlerFunc`
- `HandlerFuncWithErrCode`
- `PageFaultHandlerFunc`
etc..


`HandlerFunc` を見ると: 
```rust
type HandlerFunc = extern "x86-interrupt" fn(_: InterruptStackFrame);
```

foreign calling convention (Rust とは別の呼び出し規約) `x86-interrupt` ってなんだ. 


2021-07-01
### The Interrupt Calling Convention

例外は関数呼び出しに似ている: CPU は呼び出された関数の最初の命令に jump して実行する. 
その後 CPU はリターンアドレスへ jump して呼び出し元関数の実行を継続する. 

しかし, 例外と関数呼び出しには大きな違いがある: 関数呼び出しは `call` 命令によって任意に呼び出すことが可能な一方, 例外はどの命令でも起こりうる. 
この結果の違いを理解するためには関数呼び出しをよく理解する必要がある. 

[呼び出し規約 calling conventions](https://duckduckgo.com/?q=%E5%91%BC%E3%81%B3%E5%87%BA%E3%81%97%E8%A6%8F%E7%B4%84&ia=web) は関数呼び出しの詳細を指定する. 
例えば, 関数の引数がどこに配置されるか, 結果をどのように返すかを指定する. 
x86_64 Linux では, C の関数には以下のルール ([System V ABI]() で指定される) が適用される:
- 最初の 6この引数はレジスタで渡される (`rdi`, `rsi`, `rdx`, `rcx`, `r8`, `r9`).
- 追加の引数はスタック上で渡される.
- 結果は `rax` と `rdx` で返される. 

Rust は C ABI に従っていない. C ABI を使いたい場合は `extern "C" fn` が必要.

### Preserved and Scratch Registers

呼び出し規約はレジスタsを２つに分ける. 
一つは preserved レジスタ, もう一つは scrach レジスタ. 

The values of preserved registers must remain unchanged across function calls.
preserved レジスタの値は 関数呼び出しを通じて不変でなければならない. なので呼び出された関数 called function (= "callee") はオリジナルの値がリターン前に復元される場合にのみこれらのレジスタを上書きすることが許容される. これらのレジスタは "callee-saved" と呼ばれる. 典型的にはこれらのレジスタを関数の開始時にスタックに保存しておいてリターンする直前に復元する. 

上とは対照的に, 呼び出される関数は scratch レジスタを復元なしに上書きすることができる. 呼び出し側が関数呼び出しの間 scratch レジスタの値を保存しておきたい場合, 関数呼び出しの前に (値をスタックに送ることで) backup and restore する必要がある. そのため scratch レジスタは "caller-saved" と呼ばれる. 

callee-saved と caller-saved のレジスタ
```
callee-saved: rbp, rbx, rsp, r12, r13, r14, r15
caller-saved: rax, rcx, rdx, rsi, rdi, r8, r9, r10, r11
```



---
QUESTION: すべてを callee or caller-saved にすることはできない? なぜ2つある
- caller-saved レジスタは別名 volatile レジスタ (callee-saved は non-volatile)
- caller-saved レジスタは call の間に必要でない 一時的な quantities を保持するために使われる
- callee-saved レジスタは call の間に必要な long-lived な値を保持するために使われる

caller/callee-saved レジスタの状態が想像できない. 関数呼出のイメージがついていない?

- caller-saved register = 呼出元退避レジスタ: 呼び出された側で勝手に使って良いレジスタ
- callee-saved register = 呼出先退避レジスタ: 呼び出すときに保存しなくても良いレジスタ

[呼出規約 - Calling Convention](http://ertl.jp/~takayuki/readings/info/no04.html) より

---


## Preserving all Registers すべてのレジスタを保存する

関数呼び出しとは対照的に, 例外はどんな命令でも起こりうる. 
更に, ほとんどの場合 生成されたコードが例外を発生させるかどうかをコンパイル時に知ることはできない. 

いつ例外が発生するかわからないため, 事前にレジスタをバックアップすることはできない. 
つまり, 例外ハンドラに **caller-saved レジスタに依存する呼び出し規約**を使うことができない. 
言い換えれば, **すべてのレジスタを保存する呼び出し規約**が必要である. 
`x86-interrupt` はそのような呼び出し規約で, すべてのレジスタの値は関数が返るときに復元される. 

## The Interrupt Stack Frame

`call` を使った通常の関数呼び出しにおいて, CPU はターゲットの関数に jump する前にリターンアドレスを push する. 
関数が return するとき, CPU はリターンアドレスを pop してそこへ jump する. 


通常の関数呼び出しのスタックフレーム: 
```
---------------- <- Old Stack Pointer
Return Address (= 呼び出し元の RIP)
---------------- <- New Stack Pointer
Stack Frame
of the Hander Function (呼び出された関数)

----------------
```

例外と割り込みハンドラの場合, リターンアドレスを push するのは適切ではない, というのは割り込みハンドラは異なる文脈 (スタックポインタや CPU flags) で実行される.

割り込みの場合は以下:
1. **スタックポインタを align**: 例外はどんな命令でも起こりうるため, スタックポインタもあらゆる値を持ちうる. しかし, CPU 命令の中には (例: SSE 命令の一部) スタックポインタが 16 byte に align しておく必要がある. 
2. **スタックを切り替え** (必要であれば): CPU 特権レベルが変更される場合, スタックの切り替えが起こる. 例えば, ユーザーモードプログラムで CPU 例外が発生する場合. 
3. **旧スタックポインタを push**: 例外が発生すると (alignment の前に) CPU はスタックポインタ (`rsp`) とスタックセグメント (`ss`) レジスタを push する. これによって割り込みハンドラからリターンするときにオリジナルのスタックポインタを復元することが可能になる. 
4. **`RFLAG` レジスタの push と更新**: `RFLAG` レジスタには様々な control and status bits が含まれる. 割り込みに入るとき, CPU はいくつかの bits を変更して, 古い値を push する. 
5. **命令ポインタの push**: 割り込みハンドラ関数に jump する前に, CPU は命令ポインタ (`rip`) と code segment (`cs`) を push する. これは通常の関数呼び出しのリターンアドレスの push と同等. 
6. **エラーコードの push** (いくつかのCPU例外で): CPU がエラーコードを push.
7. **割り込みハンドラを呼び出す**: CPU はアドレスと割り込みハンドラ関数の segment descriptor を IDT (Interrupt Descriptor Table) から読み出す. そしてそのハンドラを呼び出す. 

```
割り込みスタックフレーム: 

--------------
Stack Alignment (variable)
--------------
Stack Segment (SS): スタックの先頭を指す
--------------
Stack Pointer (RSP): SS からのオフセットで表す
--------------
RFLAG
--------------
Code Segment (CS): コードセグメントの先頭を指す
--------------
Instruction Pointer (RIP): CS からのオフセットで表す
--------------
Error Code (optional)
--------------
Stack Frame of the Handler Function
--------------
```

`x86_64` クレートでは, 割り込みスタックフレームは `InterruptStackFrame` 構造体で表現される.

---
QUESTION: 割り込みとスタックについて理解できていない. 
割り込みスタックフレームがどのように使われているか理解していない. 


割り込みと関数呼び出しの違い:
1. 戻り先アドレスに加え, CPU の内部状態をスタックに格納する必要がある.
2. 割り込み同士の優先順位が決まっている. 

Q: 上の割り込みスタックフレームで, 他のレジスタはどこに保管する?


[スタックと割り込み - プログラムが動く仕組みを知ろう ページ6](http://www.kumikomi.net/archives/2008/07/15stack.php?page=6)

- Code Segment (CS): コード用のセグメントレジスタ. 命令ポインタ RIP は常にこのセグメントレジスタを使用. (命令ポインタは CS:RIP でアドレスを指定する)
- Stack Segment (SS): スタック用のセグメントレジスタ. RSP, RBP によるメモリ参照時はこのセグメントレジスタが使用される. 

[8086 のレジスタ](http://www.tamasoft.co.jp/lasm/help/lasm1to2.htm)


---

## Behind the Scenes

他に `x86-interrupt` 呼び出し規約について知っておくと良いこと. 

- **引数の扱い**: ほとんどの呼び出し規約は引数がレジスタで渡されることを想定している. これは例外ハンドラでは不可能, スタックにレジスタの値をバックアップするまでそのレジスタを上書きできないから. その代わり, `x86-interrupt` 呼び出し規約は引数がすでに特定のオフセットでスタックに存在することを想定している. 
- **`iretq` を使ってリターンする**: 割り込みスタックフレームは通常関数呼び出しのそれとはまったく異なっているので, 通常の `ret` 命令ではリターンできない. 代わりに `iretq` 命令が使われる必要がある. 
- **エラーコードの扱い**: エラーコード stack alignment を変更し, リターンの前に pop される必要がある. 
- **Aligning the stack**: 16-byte stack alignment が必要な命令 (SSE 命令など) がいくつかある. CPU は 例外が発生したとしてもこの alignment を保証するが, いくつかの命令では, CPU がエラーコードを発するときにその alignment を破壊する. 

これらの問題はすべて, `x86_64` クレートが処理している. 

## Implementation
`src/lib.rs`: 
```rust
pub mod interrupts;

pub fn init() {
    interrupts::init_idt();
}
```

`src/interrupts.rs`: 
```rust

use lazy_static::lazy_static;

lazy_static! {
    // `lazy_static!` では `static ref` で定義する (マクロがそういう仕様になっている)
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}
```

`src/main.rs`:
```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init(); // new

    // `int3` 命令を発生させる. 
    // CPU は, IDT から breakpoint ハンドラ関数を読み出して実行しようとする. 
    x86_64::instructions::interrupts::int3(); // new
    // ハンドラ関数を実行後, 復帰して実行を継続する. 

    // as before
    #[cfg(test)]
    test_main();

    println!("It did not crash!"); // 実行されるはず
    loop {}
}
```
### Adding a Test

`src/lib.rs`:
```rust
#[cfg(test)]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    init();      // IDT のセットアップ
    test_main();
    loop {}
}
```

`src/interrupts.rs`:
```rust
#[test_case]
fn test_breakpoint_exception() {
    // breakpoint 例外が起こる. 
    // 上記の通り, CPU は IDT を読み込んで breakpoint 例外のハンドラ関数を実行しようとする
    // 
    x86_64::instructions::interrupts::int3();
}
```

---
2021-07-28 MTG 追記

caller-saved は (必要であれば) 呼出先への jump 前にレジスタをスタックへ push. 例としては `rax`: x86 では返り値を保持するため, 必然的に caller-saved となる.

callee-saved の場合は (必要であれば) jump 後にレジスタをスタックへ push. 例としては `rbp`. 

割り込みスタックフレームの形成 (= 一部レジスタの保存) は CPU が割り込み時にスタックへの push を行う (caller-saved でも callee-saved でもない). 

SS, CS セグメントレジスタは x86_64 では形骸化 (権限属性を保持するのみ): 64bit では 2^64+ のアドレスを一度に指定可能 (RSP は 64bit の長さを持つので, ベースとオフセットという形式を取る必要がなくなった)





















