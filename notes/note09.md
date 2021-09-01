[Paging Implementation](https://os.phil-opp.com/paging-implementation/)

# Paging Implementation のメモ

## Accessing Page Tables

カーネルからページテーブルへアクセスすることは簡単ではない. 
問題を理解するため 4段ページテーブルを考える. 

ここで重要なのは, 各ページエントリは次のテーブルの **物理アドレス** を保存していること. 
これによって次のページのアドレスを変換することを回避し, パフォーマンス向上やアドレス変換による無限ループの可能性を回避している. 

問題はカーネルが仮想アドレス上で動いているため, カーネルから物理アドレスへ直接アクセスできないこと. 
例えば, カーネルが `4KiB` アドレスへアクセスすると, 物理アドレスではなく仮想アドレスの `4KiB` へアクセスすることになる. 

そのため, ページテーブルフレーム (= ページテーブルが保存されているフレーム) へとアクセスするためには, いくつかの仮想アドレスをフレームへとマップする必要がある. 
任意のページテーブルフレームへのアクセスを行うためのマッピングをつくる方法は複数ある. 

### Identity Mapping 

簡単な解決方法は to identity map all page tables. 
(仮想アドレスと物理アドレスを同一のものにすること (?))

QEUSTION: identity mapping はどう訳すべき

```
Virtual Memory
---------------- 0KiB
-> 0-4KiB Frame
---------------- 4KiB
-> 4-8KiB Frame
---------------- 8KiB
...
---------------- 32KiB
-> 32-36KiB Frame
---------------- 36KiB

Physical Memory
---------------- 0KiB
Page Table 1
---------------- 4KiB
Page Table 2
---------------- 8KiB
...
---------------- 32KiB
Page Table 3
---------------- 36KiB
```

これならページテーブルフレームへ仮想アドレスでアクセス可能となる. 

identity mapping ではページテーブルの物理アドレスは, 仮想アドレスとしても valid で, ページテーブルへのアクセスが容易になる. 

---
QUESTION: bootloader のページングはどうアクセスしていた? どう動いていた?

参考:
- [kernel が実装する paging の用途](https://qiita.com/kahirokunn/items/c58784473c97534cf76d#kernel%E3%81%8C%E5%AE%9F%E8%A3%85%E3%81%99%E3%82%8Bpaging%E3%81%AE%E7%94%A8%E9%80%94)

---

しかし, この方法では仮想アドレスで広い連続メモリ領域を確保することが難しくなる (ところどころページテーブル専用のマッピングに使われるため). 
確保できたとしても, フラグメンテーションのように無駄になるメモリ領域が大きくなる. 

同様に, 新しいページテーブルの生成も難しくなる, というのも対応する仮想アドレスが使用されていないような物理アドレスを見つける必要があるから. 

### Map at a Fixed Offset

identity mapping で仮想アドレスが細切れになってしまう問題を回避するため, ページテーブルマッピングに別のメモリ領域を使うことができる. 
identity mapping page table frames ではなく, ページテーブルフレームを仮想アドレス空間の固定オフセットにマップする. 

このアプローチにも欠点があり, それは新しいページテーブルを生成するときは常に新しいマッピングを生成する必要があることだ. 
また, ほかのアドレス空間のページテーブルへのアクセスもできない, これは新しいプロセスを作る場合に不便. 

QUESTION: 上はどういうこと?

### Map the Complete Physical Memory

上の問題は, ページテーブルフレームだけでなく, **すべての物理メモリをマッピングする** ことで解決可能. 

このアプローチではカーネルは任意の物理メモリへアクセスすることが可能になる. 
The reversed virtual memory range は以前 (map at a fixed offset による方法) と同じサイズで, マップされていないページがなくなる. 

このアプローチの欠点は, 物理アドレスのマッピングを保存するための追加のページテーブルが必要となること. このページテーブルはどこかに保存される必要があり, つまり物理メモリの一部を使用することになるが, これはメモリが少ない機器では問題となる可能性がある. 

しかし, x86_64 ではサイズが 2MiB ある大きなページをこのマッピングに使用可能. 

### Temporary Mapping 

物理メモリ容量が小さい機器に対しては, アクセスされるときの**一時的にのみページテーブルフレームをマップする**こともできる. 
一時マッピングをつくるのに必要なのは一つの identity-mapped level 1 table のみ


> The level 1 table in this graphic controls the first 2 MiB of the virtual address space. This is bexause it is reachable by starting at the CR3 register and following the 0th entry in the level 4, level 3, and level 2 page tables. The entry with index `8` maps the virtual page at address `32 KiB` to the physical frame at address `32 KiB`, theby identity mapping the level 1 table itself. The graphic shows this identity-mapping by the horizontal arrow at `32 KiB`. 

図中の level 1 テーブルは最初の仮想アドレス空間の 2MiB をコントロールする. 
これは, CR3 レジスタから level 4 -> 3 -> 2 とつながっており到達可能なため. 
index `8` のエントリはアドレス `32KiB` の仮想ページを物理フレームのアドレス `32KiB` へとマップしており, つまり level 1 テーブルそのものを identity mapping している. 

identity mapping された level 1 テーブルへと書き込みを行うことで, カーネルは 511 の一時的なマッピングを生成可能となった. 

一時的なマッピングによって, 下のプロセスで任意のページテーブルフレームへアクセス可能となる: 
- identity-mapped level 1 table 内部の空いているエントリを探す. 
- アクセスしたいページテーブルの物理フレームをマップする. 
- 仮想ページから目標のフレームへとアクセスする. 
- temporary mapping を削除することでエントリを使われていない状態へ戻す. 

-> **任意のページテーブルフレームへのアクセスが, 小さいメモリ専有で可能となることが利点.** 

この手法では同じ 512 の仮想ページをマッピングの生成に用いるので, 4KiB の物理メモリしか必要にならない. 欠点は若干複雑なこと.

### Recursive Page Tables

別のアプローチは, 追加のページテーブルを必要とせず, **ページテーブルを再帰的にマップする**こと. 
このアプローチの背後には level 4 ページテーブルのいくつかのエントリを level 4 ページテーブル自身にマップする考えがある. 


CPU が変換時にこのエントリに従うと, level 3 テーブルへ到達せずに level 4 テーブルへ戻る. 
これは再帰関数に似ており, そのため再帰的ページテーブルと呼ばれる. 
重要なのは CPU が level 4 テーブルの各エントリが level 3 テーブルを指すことを前提としているため, level 4 テーブルが level 3 テーブルとして扱われること. 
これは x86_64 のページテーブルが同じレイアウトだからこそ機能する. 

実際の変換を始める前に再帰エントリを一回または複数回たどることで, CPU がたどるレベルの数を省略できる. 
例えば, 再帰エントリを一度踏んでから level 3 テーブルへと進んだ場合, CPU は level 3 テーブルを level 2 table と考える. 
さらに進むと, level 2 テーブルを level 1 テーブルとして扱い, さらに level 1 テーブルをマップされたフレームとして扱う. 
つまり, level 1 テーブルを読み書きすることが可能となる. 
同様に, 再帰を 2回行うことで level 2 テーブルを読み書きできるようになる. 
同じように, level 3, 4 テーブルの読み書きも可能となる. 

再帰ページングの欠点: 
- 仮想メモリを大きく専有する (512GiB). これは 48bit アドレス空間では大きな問題とならないが, キャッシュ的にはよろしくない. 
- It only allows accessing the currently active address space easily. Accessing other address spaces is still possible by changing the recursive entry, but a temporary mapping is requireed for switching back. 
- この仕組みは x86 のページテーブルフォーマットに依存しているため, 他のアーキテクチャでは機能しない可能性もある. 

参考: 
- [Recursive Mapping](https://os.phil-opp.com/page-tables/#recursive-mapping)

---
### memo: can と may (could と might)
- (may と比較すると) can は the physical or mental ability to do something に言及する場合に使用される. 
- (can と比較すると) may は authorization or permisson to do somethung に言及する場合

ただ, かなり混同されるらしく, can にも許可を求める意味があるらしい. 

can: 
- to be able to
- to be allowed to
- used to request something

may: 
- (used to exporess possibility)
- used to ask or give permission
- used to introduce a with or a hope

---

## Bootloader Support

これらの手法はページテーブルの調整が必要となる. 
例えば, 物理メモリへのマッピングは level 4 テーブルが再帰的にマップらせることが必要. 
問題は既存のページテーブルへのアクセス無しに必要となるマッピングを作成することができないこと. 

つまり, ブートローダー (ブートローダーはカーネルを走らせるページテーブルを作成する) の助けが必要となる. 
ブートローダーはページテーブルへアクセス可能であるため, 必要なマッピングを生成できる. 
現在の実装では, `bootloader` クレートは cargo を通して機能を提供している.   

- `map_physical_memory` 機能ですべての物理メモリを仮想アドレス空間へとマップする. そのため, カーネルはすべての物理アドレスにアクセス可能となり, Map the Complete Physical Memory の手法が可能となる. 
- `recursive_page_table` 機能を使うことで, bootloader が再帰の level 4 ページテーブルエントリを作成する. これによってカーネルからページテーブルへのアクセスが可能となる. 

簡単でアーキテクチャ依存でない前者の手法を使う. 

```toml
[dependencies]
bootloader = { version = "0.9.8", features = ["map_physical_memory"]}
```

bootloader は物理メモリ全体を使用されていない仮想アドレス範囲へとマップする. 
カーネルへ仮想アドレス範囲を伝えるため, bootloader は boot information 構造体を渡す. 

### Boot Information

`bootloader` クレートは, カーネルへと渡す情報すべてを含んだ `BootInfo` 構造体を定義している. 

`map_physical_memory` 機能を有効にすると, `memory_map` と `physical_memory_offset` のフィールドが利用可能となる: 
- `memory_map` フィールドには利用可能な物理メモリの overview が含まれる. これによってカーネルはどの程度物理メモリがシステムで利用できるか, またどのメモリ領域が VGA hardware 等に予約されているかを把握できる. メモリマップは BIOS や UEFI からクエリ可能であるが, しかしそれはブートプロセスのごく初期段階に鍵られっる. このため, その初期段階以降のメモリマップはブートローダーによって提供される必要となる. 
- `physical_memory_offset` によって物理メモリマッピングに使われる仮想アドレスの開始アドレスが分かる. 

`src/main.rs`: 
```rust
use bootloader::BootInfo;

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! { // `BootInfo` を追加. 
    […]
}
```

### The `entry_point` Macro

`_start` 関数は外部である bootloader から呼び出されるため, 関数シグネチャの検証が発生しない. 
つまり任意の引数がコンパイラエラーなく取られることもあり得るが当然これは危険. 

function signature: 
> プログラミングで, メソッドや関数の, 名前および引数の数や型の順序などの組み合わせ. 戻り値の型を含む場合もある. 

entry point 関数が適切なシグネチャを持つことを確認するため, `bootloader` クレートは型検証された Rust 関数をエントリポイントとして定義する方法を提供する `entry_point` マクロを用意している. 

`src/main.rs`: 
```rust
use bootloader::{BootInfo, entry_point};

entry_point!(kernel_main); // Rust 関数を型検証しつつ entry point として定義する. 

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    […]
}
```

`src/lib.rs`: 
```rust
#[cfg(test)]
use bootloader::{entry_point, BootInfo};

#[cfg(test)]
entry_point!(test_kernel_main);

/// Entry point for `cargo test`
#[cfg(test)]
fn test_kernel_main(_boot_info: &'static BootInfo) -> ! {
    // like before
    init();
    test_main();
    hlt_loop();
}
```

## Implementation

物理アドレスへのアクセスが可能になったので, 自分のページテーブルを実装可能となった. 
まず, カーネルをその上で動かしている現在有効なページテーブルをみる. 
次に, 与えられた仮想アドレスがマップされている物理アドレスを返す変換関数を作成する. 
最後に, そのページテーブルを変更して新しいマッピングを作成する. 

### Accessing the Page Tables

カーネルから `CR3` のアドレスへアクセスしようとすると, アクセスできない 
(`CR3` のさすアドレスは, カーネルからアクセスしようとすると CPU に仮想アドレスだと認識されて変換作業が行われるため. 
CPU が変換作業のため `CR3` にアクセスした場合は, CPU は物理アドレスだと解釈するため問題ない). 


`src/memory.rs`: 
```rust
use x86_64::{
    structures::paging::PageTable,
    VirtAddr,
};

pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr(); // page table への生ポインタ
    
    // 生ポインタの参照外しの可変参照 (`&mut` として扱うため)
    &mut *page_table_ptr // unsafe: 生ポインタの参照外しが発生
}
```

level 4 テーブルを確認してみる. 

`src/main.rs`: 
```rust
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::memory::active_level_4_table;
    use x86_64::VirtAddr;

    println!("Hello World{}", "!");
    blog_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let l4_table = unsafe { active_level_4_table(phys_mem_offset) }; // level 4 table をとってくる

    for (i, entry) in l4_table.iter().enumerate() {
        // active なエントリを表示
        if !entry.is_unused() {
            println!("L4 Entry {}: {:?}", i, entry);
        }
    }

    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    blog_os::hlt_loop();
}
```
### Translating Address

仮想アドレスを物理アドレスへと変換するため, 4 つのページテーブルを通過する必要がある. 
それを実行する関数を作成する. 

`src/memory.rs`: 
```rust
use x86_64::PhysAddr;

// - 仮想アドレスから物理アドレスへの変換を行う関数
// - unsafe の範囲を小さくするため, safe なインナー関数をつくる
pub unsafe fn translate_addr(addr: VirtAddr, physical_memory_offset: VirtAddr)
    -> Option<PhysAddr>
{
    translate_addr_inner(addr, physical_memory_offset)
}

fn translate_addr_inner(addr: VirtAddr, physical_memory_offset: VirtAddr)
    -> Option<PhysAddr>
{
    use x86_64::structures::paging::page_table::FrameError;
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read(); // L4 テーブルへのアドレス

    // 変換したい仮想アドレスを L1-4 の各テーブルの index の配列にする
    let table_indexes = [
        addr.p4_index(), addr.p3_index(), addr.p2_index(), addr.p1_index()
    ];
    let mut frame = level_4_table_frame;

    // 各ページテーブルでの処理
    for &index in &table_indexes {
        // テーブルの物理フレームをマップした仮想アドレス
        let virt = physical_memory_offset + frame.start_address().as_u64();
        // テーブルの生ポインタ
        let table_ptr: *const PageTable = virt.as_ptr();
        // テーブル本体 (生ポインタの参照外しがあるため unsafe)
        let table = unsafe {&*table_ptr};

        let entry = &table[index];
        frame = match entry.frame() {
            Ok(frame) => frame,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => panic!("huge pages not supported"),
        };
    }

    // 仮想アドレスに対応する物理アドレスを返す (frame の先頭アドレス + frame 内のオフセット)
    Some(frame.start_address() + u64::from(addr.page_offset()))
}
```

実際に変換してみる.

`src/main.rs`: 
```rust
// in src/main.rs

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::memory::translate_addr;

    […]

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);

    let addresses = [
        0xb8000, // VGA text buffer のアドレス
        0x201008, // ある code page のアドレス
        0x0100_0020_1a10, // ある stack page のアドレス
        boot_info.physical_memory_offset, // 物理アドレス 0番地にあたる仮想アドレス
    ];

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        let phys = unsafe { translate_addr(virt, phys_mem_offset) };
        println!("{:?} -> {:?}", virt, phys);
    }

    […]
}
```


### Using `OffsetPageTable`

仮想アドレスから物理アドレスへの変換は OS カーネルの基本的なタスクなので, `x86_64` クレートもその抽象を提供している. 
その実装のほうが当然優れているのでそれを使う. 

その abstraction のベースには 2つのトレイトがある: 
- `Mapper` トレイトはページを操作する関数を提供する. 
- `Translate` トレイトは複数のページサイズを扱うための関数を提供する. 

`src/memory.rs`: 
```rust
// 物理アドレスが仮想アドレスに一定のオフセットでマップされているときに使える page table
use x86_64::structures::paging::OffsetPageTable;

// `OffsetPageTable` を取得する 
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

// `active_level_4_table` は private にする
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{…}
```

## Creating a new Mapping

新しいマッピングを生成する関数をつくる. 


### A `create_example_mapping` Function

任意の仮想ページを `0xb8000` へとマップする  `create_example_mapping` を作成する. 

VGA text buffer のフレームを選んだのは, メモリへの書き込みが簡単に検証可能だから. 

`src/memory.rs`: 

```rust
use x86_64::{
    PhysAddr,
    structures::paging::{Page, PhysFrame, Mapper, Size4KiB, FrameAllocator}
};

/// Creates an example mapping for the given page to frame `0xb8000`.
pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    // Result<_> 型
    // page table で特定の `page` を `frame` へとマップする. 
    let map_to_result = unsafe {
        mapper.map_to(page, frame, flags, frame_allocator)
    };
    map_to_result.expect("map_to failed").flush();
}
```

### A dummy `FrameAllocator`
`create_example_mapping` を呼び出すため `FrameAllocator` を実装する必要がある. 

`src/memory.rs`: 
```rust
pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}
```

### Choosing a Virtual Page

`FrameAllocator` の `allocate_frame` は常に `None` を返すので, 追加のページテーブルフレームが不要な場合のみ機能する. 
追加のページテーブルフレームが必要な場合・不要な場合を理解するため, 例を考える. 


ページテーブルは物理メモリフレームへと保存される. 
仮想アドレススペースはアドレス `0x803fe0000` に, マップされた一つページを含んでいる. 
このページをフレームへと変換するため, CPU は 4つのページテーブルを経由する. 

新しいマッピングを作成する何度はマップしようとする仮想ページによって異なる. 
もっとも簡単な場合では, level 1 ページテーブルがすでに存在しており, エントリを書き加えるだけ. 
もっとも難しい場合では, アドレスに対応する level 3 ページテーブルが存在せず, 新しい level 3,2,1 のページテーブルを作成する必要が出てくる. 

`EmptyFrameAllocator` で `create_example_mapping` 関数を呼ぶためには, すべてのページテーブルがすでに存在しているようなページでなければならない. 
そのようなページを見つけるためには, ブートローダが仮想アドレス空間の最初の 1メガバイトに自身をロードするという事実を利用することができる. 
つまりこの領域にはすべてのページのための level 1 テーブルが存在しているといえる. 
なので, この領域, 例えばアドレス `0` へマッピングを行う. 
通常, このページは使われない (ヌルポインタの参照外しが page fault を起こすようにするため) ため, ここを使う. 

### Creating the Mapping

`create_example_mapping` 関数に必要なパラメータはそろったので, `kernel_main` 関数から呼び出してみる. 

`src/main.rs`: 
```rust
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::memory;
    use x86_64::{structures::paging::Page, VirtAddr}; // import

    […] // hello world and blog_os::init

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset); // bootinfo から物理メモリマッピングの offset を取得
    let mut mapper = unsafe { memory::init(phys_mem_offset) }; // page_table の初期化
    let mut frame_allocator = memory::EmptyFrameAllocator; // 空の frame fllocator を使用 -> あらかじめページテーブルの存在するページのみを呼び出す必要がある. 

    // 使われていないページをマップする.  
    let page = Page::containing_address(VirtAddr::new(0)); // 仮想アドレス `0` は使われていない (entry が存在しない), 且つ, 仮想アドレス `0` はブートローダーのロードのためにページテーブルが存在する. 
    memory::create_example_mapping(page, &mut mapper, &mut frame_allocator); // 仮想アドレス `0` を VGA buffer へとマップする. 

    // VGA buffer へ `New!` を書き込むことで, 適切に `create_example_mapping` が動いているかを確認する. 
    let page_ptr: *mut u64 = page.start_address().as_mut_ptr(); // 仮想アドレス `0` の page
    unsafe { page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e)}; // VGA buffer が println で `New!` をスクリーンから消してしまうので 400 の offset を追加

    […] // test_main(), "it did not crash" printing, and hlt_loop()
}
```

### Allocating Frames

ここまでは新しいページテーブルなし (`EmptyFrameAllocator`) でページをマップしていた. 
でもやはり新しいページテーブルを作りたい. 

新しいページテーブルを作るためには適切な frame allocator が必要となる. 
そのために `BootInfo` 構造体として渡される `memory_map` を使う. 

`src/memory.rs`:
```rust
use bootloader::bootinfo::MemoryMap;

/// A FrameAllocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }
}
```

この構造体には 2つのフィールドがあり, `'static` 参照は


memory map は BIOS/UEFI ファームウェアが提供する. 
この機能は boot process のごく初期段階のみでしか query 可能ではないので, bootloader がそれぞれの関数を呼び出している. 
memory map は `MemoryRegion` 構造体のリストを保持しており, `MemoryRegion` は各メモリ区域の 開始アドレス, 長さ, 属性 (e.g. unused, reserved, etc.) から構成されている. 

`init` 関数は `BootInfoFrameAllocator` を与えられた memory map で初期化する. 
`next` フィールドは `0` で初期化され, 各 frame allocation で増加して 同じフレームを 2度返すことを防ぐ. 
bootloader から提供された memory map にある使用可能なフレームがすでに使われているか分からないため, `init` 関数は `unsafe` である必要がある. 

### A `usable_frames` Method

`FrameAllocator` トレイトを実装する前に, memory map を使用可能フレームの iterator に変換する補助メソッドを追加する. 

```rust
use bootloader::bootinfo::MemoryRegionType;

impl BootInfoFrameAllocator {
    /// memory map から usable frame を取り出して iterator へと変換
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // usable region を取り出す
        let regions = self.memory_map.iter();
        let usable_regions = regions
            .filter(|r| r.region_type == MemoryRegionType::Usable);
        // usable region から (開始物理アドレス..終了物理アドレス) を取り出す
        let addr_ranges = usable_regions
            .map(|r| r.range.start_addr()..r.range.end_addr());
        // usable region の range を 4096B ごとに分割し, その開始アドレスに変換する. 
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // 取得した開始物理アドレスから物理フレームへ変換
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

```

### Implementing `FrameAllocator` Trait
`FrameAllocator` トレイトを実装する. 

`src/memory.rs`: 
```rust
unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        // まだ allocate していない usable_frame を取得
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
```

### Using the `BootInfoFrameAllocator`

`kernel_main` で同じことを行い, 正常に機能するか確認する. 

`src/main.rs`: 
```rust
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::memory::BootInfoFrameAllocator;
    […]
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_map)
    };
    […]
}
```


### TODO: memory map とページテーブルの違い
- memory map: BIOS/UEFI ファームウェアとブートローダがつくる. 構造はどうなっている?
- page table: カーネル向けの page table はブートローダがつくる. 

### TODO: `impl Trait` と `dyn Trait` の違い

どちらも trait で型を隠蔽する

`impl Trait`
- `impl Trait` は型でない. 匿名の型を表すための糖衣構文で, `impl Trait` はコンパイル時に別の型として翻訳される
- 静的に解決 -> `impl Trait` で扱う型がコンパイル時に決定されるなら使える

`dyn Trait`
- 動的に解決 -> `dyn Trait` で扱う型がコンパイル時に決定できない時に使用する
- 型を決定できない -> サイズが不明 -> `Box<dyn Trait>` で使う. 
- 


```rust
use std::iter;

// nの倍数を列挙 (コンパイルエラー)
fn multiples_of(n: i32) -> impl Iterator<Item=i32> {
    if n == 0 { //~ERROR if and else have incompatible types
        iter::once(0)
    } else {
        (0..).map(move |m| n * m)
    }
}
```

参考: 
- [安定化間近！Rustのimpl Traitを今こそ理解する](https://qnighy.hatenablog.com/entry/2018/01/28/220000)
- [Rustのimpl Traitが使えそうで使えない場所](https://ironoir.hatenablog.com/entry/2021/01/11/174735)








