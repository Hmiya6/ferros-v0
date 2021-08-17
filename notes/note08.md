[Introduction to Paging](https://os.phil-opp.com/paging-introduction/)

# Introduction to Paging のメモ

ページングは基本的なメモリ管理スキーム


## Memory Protection

OS の重要な役割として, プログラムの隔離がある. 
これを達成するため, OS はハードウェアの機能を活用して他のプロセスからアクセスされないメモリ空間を保証する. 
ハードウェアと OS 実装によって異なったアプローチがある. 

たとえば, ARM Cortex-M プロセッサ群の中には Memory Protection Unit (MPU) を持つものもあり, MPU によって少数 (8程度) のメモリ領域を異なるアクセス権限で定義可能 (no access, read-only, read-write). 
毎メモリアクセスにおいて MPU はアドレスが適切なアクセス権限であることを検証し, 適切でない場合は例外を投げる. 
各プロセス変更において領域とアクセス権限を変更することで, OS は各プロセスが自分のメモリにのみアクセスすることを保証し, またプロセスの隔離を行う. 

x86 においては, ハードウェアは異なった 2つのメモリ保護アプローチをサポートしている: 
- セグメンテーション
- ページング

## Segmentation

セグメンテーションは 1978年に導入され, もともとはアドレス可能な adressable メモリの量を増加させるためだった. 
その当時, CPU は 16bit アドレスを使用しており, adressable memory の量は最大 64KiB だった. これを増加させるため, 追加で segment registers が導入された, 各 segment register は offset address を保持した. 

セグメントレジスタは CPU によって自動で選択され, それはメモリアクセスの種類による. 
命令をとってくるためには code segment `cs` が使われ, スタックの操作には stack segment `ss` が使われる. 
他の命令が data segment `ds` か extra segment `es` を使用する. 
後から追加のレジスタ `fs`, `gs` が加えられたが, いまは自由用途で使われている. 

初期のセグメンテーションでは, セグメントレジスタは直接オフセットを保持して, アクセス制御は行われていなかった. 
これは後に protected mode の導入により変更された. 
CPU が protected mode で動くとき, segment descriptor は local or global descriptor table 内のインデックスを保持し, local/global descriptor table がオフセットアドレスを保持し, それに加えてセグメントのサイズやアクセス権限も保持する. 
別の global/local descriptor table をそれぞれのプロセスにロードすることで OS はプロセス群を隔離する. 
QUESTION: 複数の descriptor table とプロセス隔離のメカニズム

### Virtual Memory

仮想メモリの概念の裏には物理ストレージ機器 (RAM など) からメモリアドレスを抽象化することがある. 
ストレージ機器へ直接アクセスするのではなく, メモリアドレスの変換がまず実行される. 
セグメンテーションのため, この変換では使用中のセグメントの offset address を加える. 

物理アドレスは unique で常にメモリ上の同じ場所を指す. 
一方, 仮想メモリは (アドレスは) 変換依存である. 

もう一つの利点はプログラムが任意の物理メモリ場所へ配置できる. OS は利用可能なメモリをすべて プログラムを再コンパイルすることなく使用できる. 


### Fragmentation

仮想・物理アドレスの差別化はセグメンテーションをさらに強力なものにする. 
しかしセグメンテーションにはフラグメンテーションの問題がある. 

```
---- 000

---- 100
Virtual Memory 1 (size 200, offset 100)
---- 300

---- 400
Virtual Memory 2 (size 100, offset 400)
---- 500

```

上図の場合, サイズには空きがあるのに, 200 サイズの仮想メモリを物理メモリに追加 map できない. 
問題は連続したメモリが必要とされ, 小さい空きメモリが使用できないことだ. 

フラグメンテーションを解決する方法の一つは, 実行を一時停止して使用メモリ移動させ, 変換をアップデートして, 実行に復帰する. 

```
--- 000
Virtual Memory 1
--- 200
Virtual Memory 2
--- 300
FREE SPACE
--- 500
```

フラグメンテーションはほとんどのシステムでサポートされていない. 
実際は x86 の 64bit モードでもサポートされない. 
代わりにページングが使われており, フラグメンテーションの問題を解決する. 

## Paging

仮想メモリと物理メモリのスペースを, 小さい固定サイズのブロックに分割するという考え方. 
仮想メモリスペースのブロックは **ページ** と呼ばれ, 物理アドレススペースのブロックは **フレーム** と呼ばれる. 
各ページは個別にフレームへマップされており, それらによってより広いメモリ区画を非連続な物理フレームで分割可能となる. 

仮想メモリをページで細かく区切って, 同じサイズで区切った物理メモリのフレームへ当てはめる.  
-> 無駄なく (フラグメンテーションの問題を生じさせることなく) 物理メモリを使用できる. 

例: 
```
Physical Memory
--- 000
Frame 1
--- 050
Frame 2
--- 100
Frame 3
--- 150

---
========

Virtual Memory 1
------- 000
Page 1 -> Frame 2
--- 050
Page 2 -> Frame 4
--- 100
Page 3 -> ...
--- 150
Page 4 -> ...
------- 200

Virtual Memory 2
------- 000
Page 1 -> Frame 1
--- 050
Page 2 -> Frame 3
------- 100

```

### Hidden Fragmentation

少数の大きく, variable なサイズのメモリ区画を用いるセグメンテーションと比較して, ページングは多数の小さい固定サイズのメモリ区画を使用する. 
各フレームは同じサイズを持つので, 小さすぎるメモリ区画もなくなり, フラグメンテーションが起こらない. 

しかしまだ内部フラグメンテーション internal fragmentation がある. 
内部フラグメンテーションが発生するのはすべてのメモリ区画が必ずしもページサイズ倍数分のサイズ (e.g. ページサイズ *1, *2, *3, ...) であることが原因となる. 
例えば, ページサイズ 50 でメモリサイズ 101 が必要な場合, 3 ページを確保する必要があるが, 無駄な 49 が生まれる. 

ほとんどの場合において内部フラグメンテーションはセグメンテーションによる外部フラグメンテーション external fragmentation よりもまし ().

### Page Tables

ページとフレームのマップ情報をどこかに保管する必要がある. 
セグメンテーションでは個々のセグメントセレクタレジスタを各使用中メモリ区画ごとに使っていたが, ページングではページがレジスタより多いのでこの方法は使えない. 
ページングは **page table** という table structure をマッピング情報を保存するために使う. 

例: 
```
Page Table of Virtual Memory 1: 
Page Frame Flags
----------------
000  100   r/w
050  150   r/w
100  200   r/w
```

各プログラムインスタンスは page table を持つ. 

使用中の table へのポインタは特別な CPU レジスタへ保存される. 
`x86` では `CR3` と呼ばれるレジスタへ保存される. 
OS は各プログラムインスタンスが実行を開始する前に正しいページテーブルへのポインタをロードする必要がある. 

各メモリアクセス時, CPU はレジスタから table pointer を読み取り table から該当するページの mapped frame を探す. 
これはハードウェアで実行され, 実行中プログラムには見えない. 
この変換プロセスを高速化するため, 多くの CPU アーキテクチャは新しい変換結果のキャッシュを持つ. 

### Multilevel Page Tables

例:
```
Virtual Memory
--- 0 000 000
#############
--- 0 000 050
.
--- 1 000 000
#############
--- 1 000 050
#############
--- 1 000 100
#############
--- 1 000 150
``` 
このとき, page table の `0 000 050` - `1 000 000` までのエントリを省略できない (必要なのは 4ページだけ). 

このメモリの無駄を削減するため, **2段のページテーブル** を使用する. 
これは別のアドレス領域に別のページテーブルを使うという考え方. 
追加テーブルは level 2 page table と呼ばれ, アドレスと level 1 page table のマッピング情報を保持する. 

```
# Level 2 Page Table (virtual address -> level 1 table)
------------------
000 000 000 -> T1
000 010 000 
...
001 000 000 -> T2
001 010 000
------------------

# Level 1 Page Table T1 ((virtual address - offset) -> frame)
------------------
000 -> 000
...
------------------

# Level 1 Page Table T2 
------------------
000 -> 100
050 -> 150
------------------

```

上の例の場合, level 2 table は `010_000` byte ごとに level 1 table へのポインタを有することになる. 
このとき, level 2 table での無駄なエントリは 100 エントリとなる. 
これは level 1 のみの場合よりも 15_000 - 20_000 エントリ無駄が削減される. 

この仕組みは多段ページテーブル 階層ページテーブル multilevel or hierarchical page table と呼ばれる. 


## Paging on x86_64

x86_64 アーキテクチャでは 4段ページテーブルが使用され, ページサイズは 4KiB である. 
各ページテーブルはレベルに拘わらず, 512 の固定サイズのエントリを保持する. 
各エントリは 8 bytes だから, 各テーブルは 512 * 8B = 4KiB. 

```
仮想アドレスの構造: 
Virtual Address:
------------ 64

------------ 48
Level 4 Index (9bit -> 512)
------------ 39
Level 3 Index (9bit -> 512)
------------ 30
Level 2 Index (9bit -> 512)
------------ 21
Level 1 index
------------ 12
Page Offset
------------ 0
```

9bit ごとに区切られているのは, 各ページテーブルが 512 のエントリを保持するから. 
下位 12bit は 4KiB ページにおけるオフセット (2^12 bytes = 4KiB). 
64-48 までの bit は使用されていない. 
つまり x86_64 は 64bit アドレスではなく 48bit アドレスを使用している. 

---
QUESTION: The lowest 12 bits are the offset in the 4KiB page. なぜ下位 12bit のオフセットが必要

---

48-64 の bit が使用されていないとしても, 好きな値をそこにおいていいわけではなく, 47bit 目の値のコピーでなければならない. 
48-64 は 5-level page table に備えてある. 

---
QUESTION: 結局どれくらい多段ページテーブルは効果的なのか. どの程度メモリを節約できる?

```
- (level 4 table) * 1 = 4KiB * 1
- (level 3 table) * 512 = 4KiB * 512
- (level 2 table) * 512 * 512 =  4Kib * 512^2
- (level 1 table) * 512 * 512 * 512 = 4KiB * 512^3
Total: 4KiB * 134_480_385 = 550_831_656_960B < 551GB

- level 1 table * 2^48 = 4KiB * 281_474_976_710_656

- 281474976710656 / 134480385 = 2_093_056.00...
- 281474976710656 - 134480385 = 281_474_842_230_271

```

これはあくまで最大の値. 
例えば 8GB 使うときは? (8GB = 8_000_000_000 bytes)
-> ln2(8_000_000_000) = 32.89.. < 33 .  
-> 33bit あれば対応可能.  
```
level-4: 1, 
level-3: 8 (30-32bit? を使用), 
level-2: 8*512, 
level-1: 8*512*512, 
Total: 4KiB * 2_101_249 = 8_606_715_904
```


QUESTION: 本当にこの計算あってる?

> mapping 32 Gib of physical memory only requires 132 KiB for page tables since only one level 3 table and 32 level 2 tables are needed.

参考:
- [What does it mean for a page table to be sparse in the context of Operating System?](https://www.quora.com/What-does-it-mean-for-a-page-table-to-be-sparse-in-the-context-of-Operating-Systems?share=1)

---

## Implementation
実は `bootloader` クレートで 4段ページングをセットアップしている. 

### Page Faults

`src/interrupt.rs`: 
```rust
lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        […]

        idt.page_fault.set_handler_fn(page_fault_handler); // page fault のハンドラを IDT に登録

        idt
    };
}

use x86_64::structures::idt::PageFaultErrorCode;
use crate::hlt_loop;

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2; // `Cr2` には page fault 時にアクセスした仮想アドレスが保存されている. 

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    hlt_loop();
}
```

level 4 page table のアドレスを確認する. 

`src/main.rs`: 
```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init();

    use x86_64::registers::control::Cr3; // `Cr3` レジスタには level 4 page table の情報が保存される. 

    let (level_4_page_table, _) = Cr3::read();
    println!("Level 4 page table at: {:?}", level_4_page_table.start_address()); // level 4 page table の開始アドレスを表示

    […] // test_main(), println(…), and hlt_loop()
}
```
















