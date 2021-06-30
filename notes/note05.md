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




















