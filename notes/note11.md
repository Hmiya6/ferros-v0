
[Allocator Designs](https://os.phil-opp.com/allocator-designs/)

# Allocator Designs のメモ

## Design Goals

allocator の役割は使用可能なヒープメモリを管理することにある. allocator は `alloc` で使用されていないメモリを返し, `dealloc` で解放されるメモリを追跡することで再使用可能にする必要がある. 
さらに重要なのは, 使用中のメモリに手を出さないこと. 

メモリ管理の正確さ以外にも, 副次的な多くの設計目標がある. 
たとえば, allocator は効率的に使用可能なメモリを使用し, fragmentation を小さくするべきである. 
さらに, 並行処理にも上手く対応し, 搭載されるプロセッサの数にもスケールするべきである. 
パフォーマンスを最大化するためには, メモリレイアウトを CPU キャッシュの観点から最適化し cache locality を向上しや false sharing を避けるようになる. 

---
TODO: キャッシュについてあまり知らない (CPU キャッシュ, メモリアドレス対応関係のキャッシュ, etc..)

CPU cache: DRAM より高速な SRAM を使ったメモリ. [キャッシュメモリ - Wikipedia](https://ja.wikipedia.org/wiki/%E3%82%AD%E3%83%A3%E3%83%83%E3%82%B7%E3%83%A5%E3%83%A1%E3%83%A2%E3%83%AA)

Translation lookaside buffer: MMU のキャッシュ. [トランスレーション・ルックアサイド・バッファ](https://ja.wikipedia.org/wiki/%E3%83%88%E3%83%A9%E3%83%B3%E3%82%B9%E3%83%AC%E3%83%BC%E3%82%B7%E3%83%A7%E3%83%B3%E3%83%BB%E3%83%AB%E3%83%83%E3%82%AF%E3%82%A2%E3%82%B5%E3%82%A4%E3%83%89%E3%83%BB%E3%83%90%E3%83%83%E3%83%95%E3%82%A1)

後で読む

---

これらの要件は allocator をとても複雑なものにしうる. 
例えば, jemalloc は 30K 行以上のコードである. 
この複雑さは一つのバグが重大な脆弱性へとつながりかねないカーネルコードでは望まれないことも多い. 
幸運なことに, カーネルコードでの allocation patterns はユーザ空間のコードに比べとても単純なので, 比較的単純な allocator 設計で十分なことが多い. 

以下ではいくつかの kernel allocator design を見ていく. 

## Bump Allocator

最も単純な allocator design は **bump allocator** (aka stack allocator). 
それは線形状 (?) にメモリを確保し, 確保されたバイトと allocation の数のみを追跡する. 
これは非常に特殊なケースでのみ使用されるが, それは厳しい制約のためである. 
bump allocator はすべてのメモリを一度に解放することしかできない. 

### Idea
bump allocator では `next` 変数を増加させることで直線状に (= 切れ目なく) メモリを確保する. 
最初は `next` はヒープのスタートアドレスと同値である. allocation を行うたびに `next` が使用されているメモリと使用されていないメモリの境界をさすように増加する. 

`next` ポインタは一方向にしか移動しないため同じメモリ区画に手を出すことはない. 
ヒープの最後に到達すると, それ以上のメモリを確保できないため, out-of-memory エラーを発生させる. 

bump allocator は allocation counter をともに実装されることが多く, counter は  `alloc` のたびに 1増加し, `dealloc` のたびに 1減少する. 
allocation counter が 0に到達した場合, それはすべての allocation が deallocate されたことを意味する. このとき, `next` ポインタはヒープの開始アドレスへともどる. 

## Implementation

`src/allocator.rs`: 
```rust
pub mod bump;
```

`src/allocator/bump.rs`: 
```rust
pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize, // ここに next が達した場合, out of memory エラー
    next: usize, // 次に allocate されるメモリアドレス (= 使用されているメモリと使用されていないメモリの境界)
    allocations: usize, // allocation が 0 になったら next をリセット
}

impl BumpAllocator {
    // 新しい bump allocator を作成
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    // bump allocator を与えられた条件で初期化
    // このメソッドは unsafe ( **プログラマ側**で範囲メモリが使われていないことを確かめる必要がある)
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}
```

`new` と `init` を分けて初期化を行っているのは `linked_list_alloator` クレートと同じインターフェースにするため. 

### Implementing `GlobalAlloc` 

`alloc` メソッドを `BumpAllocator` に実装する

`src/allocator/bump.rs`: 
```rust

use alloc::alloc::{GlobalAlloc, Layout};

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // TODO alignment and bounds check
        let alloc_start = self.next;
        self.next = alloc_start + layout.size();
        self.allocations += 1;
        alloc_start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        todo!();
    }
}
```

---
TODO: `*mut u8` で生ポインタを作成しているが `*mut u64` でなくてもいいのか

```rust
fn main() {
    let us = std::usize::MAX;
    let ptr = us as *mut u8;
    println!("{:?}", ptr); // 0xffffffffffffffff
    // OK
}
```

`*mut u64` をどこかで使っている覚えがあったが, 使い分けはどうしているのか?

例: 
```rust
// in `main.rs::kernel_main`
let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
```

---

メモリの境界やアラインメントの確認を行っていないため, これは unsafe. 

このコードはコンパイルエラーになる: 
```
error[E0594]: cannot assign to `self.next` which is behind a `&` reference
  --> src/allocator/bump.rs:29:9
   |
29 |         self.next = alloc_start + layout.size();
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `self` is a `&` reference, so the data it refers to cannot be written
```

非可変参照で `self` を取っているので `self.next` に書き込みできない. 
引数での `&self` (非可変参照) は, `GlobalAlloc` のメソッドが定めているのでここは変更できない. 

#### `GlobalAlloc` and Mutability

なぜ `GlobalAlloc` トレイトメソッドが (`&mut self` ではなく) `&self` を引数にとるのかを理解する. 

global heap allocator は `GlobalAlloc` トレイトを実装した `static` に `#[global_allocator]` 属性を追加して決定される. 
static 変数は Rust では不変なので, `&mut self` をメソッドが受け取ることができない. 
このために `GlobalAlloc` のすべてのメソッドは immutable な `&self` 参照のみをとる. 

幸いなことに `&self` から `&mut self` を取り出す方法がある. 
同期の内部可変性と `spin::Mutex` で allocator を wrap することができる. 

memo: `BumpAllocator` に  `impl` を実装するだけでなく, `Arc<Mutex<BumpAllocator>>` にも `impl` は実装可能. 

#### A `Locked` Wrapper Type

`spin::Mutex` 型を使って, `GlobalAlloc` trait を bump allocator に実装する. 

他のクレートで定義された型の trait implementation はコンパイラエラーになる: 
```
error[E0117]: only traits defined in the current crate can be implemented for arbitrary types
  --> src/allocator/bump.rs:28:1
   |
28 | unsafe impl GlobalAlloc for spin::Mutex<BumpAllocator> {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^--------------------------
   | |                           |
   | |                           `spin::mutex::Mutex` is not defined in the current crate
   | impl doesn't use only types from inside the current crate
   |
   = note: define and implement a trait or new type instead
```
---
TODO: なぜだめなのか -> ちゃんと理由がある. 
[Only traits defined in the current crate - The Rust Programming Language Forum](https://users.rust-lang.org/t/only-traits-defined-in-the-current-crate/8227), 
[Can not implement trait from another crate for generic type from another crate parameterized with local type - stackoverflow](https://stackoverflow.com/questions/29789877/can-not-implement-trait-from-another-crate-for-generic-type-from-another-crate-p)

> and the compiler can't ensure that there will be no conflicting implementations in other crates.

---

自分で型を作ればいいのでそうする. 

`src/allocator.rs`: 
```rust
/// A wrapper around spin::Mutex to permit trait implementations.
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> { // `MutexGuard` は RAII の考えを mutex へと適用したもの (drop されたら unlock される)
        self.inner.lock() // lock() -> MutexGuard
    }
}

```

#### Implemetation for `Locked<BumpAllocator>`

`Locked` 型は このクレートで定義されるので `GlobalAlloc` を実装可能. 

```rust
use super::{align_up, Locked};
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.lock(); // get a mutable reference

        let alloc_start = align_up(bump.next, layout.align()); // 適切な alignment にそろえる
        let alloc_end = match alloc_start.checked_add(layout.size()) { // usize をオーバーフローしないか調べる
            Some(end) => end,
            None => return ptr::null_mut(),
        };

        if alloc_end > bump.heap_end {
            ptr::null_mut() // メモリ範囲外なのでエラー (ヌルポインタ).
        } else {
            bump.next = alloc_end;
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut bump = self.lock(); // get a mutable reference

        bump.allocations -= 1;
        if bump.allocations == 0 {
            bump.next = bump.heap_start;
        }
    }
}
```

`align_up` 関数の実装について: 

1. 基本的な実装方法
`src/allocator.rs`: 
```rust
fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;
    if remainder == 0 {
        addr // そのまま
    } else {
        addr - remainder + align // 修正
    }
}
```

2. 高速な実装方法
```rust
/// `align` が 2^n であることが必要
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
```

```
addr: 0b1101
align:0b1000 とすると, 

addr + aligh - 1 = 0b10101 - 1 = 0b10100 //  ... (I)
!(align - 1) = !(0b0111) = 0b1...1000 // ... (II)

(I) & (II) = 0b10000

```

TODO: ビット演算を使って高速化する他の例はある?

### Using It

`src/allocator.rs`: 
```rust
use bump::BumpAllocator;

#[global_allocator]
static ALLOCATOR: Locked<BumpAllocator> = Locked::new(BumpAllocator::new());
```

### Discussion
bump allocation の最大の利点は非常に高速なこと. 
他の allocator では適切なメモリブロックの捜索と `alloc`/`dealloc` 時の追跡を行う必要があるのとは対照的に, bump allocator は数行のアセンブリ命令にまで最適化されうる. 
これによって bump allocator は allocation performance を最適化するために使い勝手がよい. 
virtual DOM library の作成時に使用されている. 

bump allocator が global allocator として使用されることはほとんどないが, その原理は arena allocation へと応用されている (arena allocation は 個々の allocation を一括して行いパフォーマンスを向上する). 

#### The Drawback of a Bump Allocator

bump allocator の主な制約の一つはすべての allocation を解放することでしかメモリを再利用できないこと. 
つまり一つの長期間生存するような allocation が適当. 

## Linked List Allocator

任意の数の free memory を追跡するためのよくある技は そのメモリ自体を backing storage として使用すること. 
解放されたメモリ区域の情報を保存することで, 追加のメモリを必要とせずに使われていないメモリ区域を追跡することが可能となる. 

最もよくある実装手法は解放されたメモリに single linked list を構築する.  

```
// 使用可能なメモリ区域の先頭に単方向の linked list をつくる
|(node) freed   | /// USED /// |(node) freed | ////// USED ////// |
```

各 list node は 2つのフィールドをもつ. 
一つはメモリ区域のサイズで, もう一つは次の使用されていないメモリ区域へのポインタ. 
この手法を使うと, すべての使用されていないメモリ区域の追跡を最初の使用されていないメモリ区域へのポインタのみ行うことが可能となる. 
この linked list は free list を呼ばれることが多い. 

### Implementation

#### The Allocator Type

`src/allocator.rs`: 
```rust
pub mod linked_list;
```

`src/allocator/linked_list.rs`: 
```rust
struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>, // Box を使いたいが, allocator の実装なので Box は使用できない. 
}
```
`&static mut` 型はポインタの裏にある所有されたオブジェクトとなる. 

```rust
impl ListNode {

    const fn new(size: usize) -> Self { // `static` な ALLOCATOR を初期化するため `const fn` を使用 
        Self { size, next: None}
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize // 
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}
```

ここで `const` 関数に `mut` 参照を使用すること (`next` フィールドに `Option<&'static mut ListNode` 型を使うこと) は unstable なので `lib.rs` に `#![feature(const_mut_ref)]` が必要となる. 

`src/allcator/linked_list.rs`: 
```rust
pub struct LinkedListAllocator {
    head: ListNode,
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    /// Initialize the allocator with the given heap bounds.
    // 必ず一度だけ呼ばれる
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size);
    }

    /// Adds the given memory region to the front of the list.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        todo!();
    }
}
```

#### The `add_free_region` Method
`add_free_region` は linked list における push 操作を提供する. 
`init` や `dealloc` で使用する. 
具体的には `dealloc` で解放したメモリ区域を linked list で追跡するために必要. 

`src/allocator/linked_list.rs`: 
```rust
use super::align_up;
use core::mem;

impl LinkedListAllocator {
    // 指定されたメモリ領域を list の先頭に加える
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // 解放されたメモリ領域が ListNode を保存できるサイズを持つか?
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        // 新しい `ListNode` をつくって保存
        // self.head --> new node --> old self.head.next
        let mut node = ListNode::new(size);
        node.next = self.head.next.take(); // `Option::take` はもともとにあった `Option` (ここでは `self.head.next`) を `None` にして中身を返す
        let node_ptr = addr as *mut ListNode;
        node_ptr.write(node);
        self.head.next = Some(&mut *node_ptr)
    }
}
```

#### The `find_region` Method

linked list のもう一つの基本的な操作は要素を見つけてそれをリストから削除すること. 
これは `alloc` メソッドを実装するために必要な操作.

`src/allocator/linked_list.rs`: 
```rust

impl LinkedListAllocator {
    // 与えられたサイズとアラインメントの解放領域を探して, list から削除し, アドレスを返す
    fn find_region(&mut self, size: usize, align: usize)
        -> Option<(&'static mut ListNode, usize)>
    {
        // 条件にあったメモリ領域を list からさがす
        let mut current = &mut self.head;
        while let Some(ref mut region) = current.next {
            // メモリ区域が alloc に適合するか
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                // list の交代処理
                // current --> region --> next から
                // current --> next に交代する
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                current = current.next.as_mut().unwrap();
            }
        }
        // 条件にあったものがなければ None
        None
    }
}

```

TODO: `ref` と `&` の違いは?

#### The `alloc_from_region` Function

`alloc_from_region` 関数は, 与えられたサイズとアラインメントに適合する領域を返す. 

`src/allocator/linked_list.rs`: 
```rust
impl LinkedListAllocator {
    fn alloc_from_region(region: &ListNode, size: usize, align: usize)
        -> Result<usize, ()>
    {
        
        // alloc する領域の開始アドレスを align する
        let alloc_start = align_up(region.start_addr(), align);
        // 領域の最後を得る
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;
        // 最後が解放領域を超えるようならエラー
        if alloc_end > region.end_addr() {
            return Err(());
        }
        
        // 解放後の残った領域に ListNode を保存できなければエラー
        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            return Err(());
        }

        Ok(alloc_start)
    }
}
```

#### Implementing `GlobalAlloc`

`add_free_region` と `find_region` で提供される操作を使うことで,  `GlobalAlloc` トレイトを実装可能となる. 
bump allocator と同じく, `LinkedListAllocator` そのものへ `GlobalAlloc` を実装するのではなく, `Locked<LinkedListAllocator>` へ実装する. 
`Locked` wrapper は spinlock も用いて内部可変性を提供しており, spinlock によって `&self` から allocator の状態を変化させることが可能となる. 

`src/allocator/linked_list.rs`
```rust
use super::Locked;
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // allocator の lock
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.lock();

        // allocation の実行
        if let Some((region, alloc_start)) = allocator.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 {
                allocator.add_free_region(alloc_end, excess_size); // allocation で余った領域を list に登録して追跡
            }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // perform layout adjustments
        let (size, _) = LinkedListAllocator::size_align(layout);

        self.lock().add_free_region(ptr as usize, size)
    }
}
```

#### Layout Adjustment

`size_align` 関数は `ListNode` を各 allocated block が保存可能であることを保証する. 
これはメモリブロックがどこかのタイミングで deallocate されるとき, そこに `ListNode` を書き込みたい. 
このメモリブロックが `ListNode`

`src/allocator/linked_list.rs`: 
```rust
impl LinkedListAllocator {
    // 
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<ListNode>()) // `ListNode` の alignment へと合わせる
            .expect("adjusting alignment failed")
            .pad_to_align(); // `ListNode` を保存可能なように
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align()) 
    }
}
```

TODO: なぜ `size_align` を必要としているか理解できていない. deallocate された区域 `NodeList` を保存するため、allocate / deallocate 時に `NodeList` に合わせる調整が必要という理解で OK? とくに align の意義が分からなくなってきた. `NodeList` の align に合わせると何がうれしい?


### Using it

`src/allocator.rs`: 
```rust
use linked_list::LinkedListAllocator;

#[global_allocator]
static ALLOCATOR: Locked<LinkedListAllocator> =
    Locked::new(LinkedListAllocator::new());
```

### Discussion

bump allocator とは対照的に、linked list allocator は解放されたメモリを再利用できるため汎用的な allocator に適している. 
しかし, 弱点もある. 
そのいくつかはシンプルな実装によるものもあるが, allocator design そのものの根本的な欠点もある. 

#### Merging Freed Blocks
この実装の主要な問題は heap を小さなブロックへと分割し続けて再び merge することがないこと. 

これを解決するために隣接する解放されたメモリブロックを merge しなおす必要がある. 

前章で使用した `linked_list_allocator` もこの merge を次の方法で実装している: 
`deallocate` 時に解放されたメモリブロックを linked list の先頭へと加えるのではなく, list を常にその開始アドレスでソートした状態に保っている. 
こうすることで, `deallocate` 呼び出し時に list 内の2つの隣接するブロックのアドレスとサイズを調べることで merge を実行可能となる. 
もちろん, deallocation 操作の速度は低下するが, これによって heap fragmentation を防ぐことが可能になる. 

#### Performance
上で扱った通り, bump allocator は非常に高速で, 数えられる程度のアセンブリ操作まで最適化できる. 
linked list allocator はそれよりもずっと低速である. 
問題は allocation request が適当なブロックを見つけるまで, linked list をすべて訪れる必要がある可能性がある点だ. 

list の長さは使用されていないメモリブロックの数に依存しているため, パフォーマンスはプログラムによってまったく異なる可能性がある. 
alloation をほとんど使わないプログラムでは allocation パフォーマンスが高い. 多くの allocation によって heap を fragment するようなプログラムでは逆に, とても小さいブロックを多く保持する長大な linked list によって allocation パフォーマンスが非常に低くくなりうる. 

このパフォーマンスの低下は簡単な実装のせいで発生するわけではなく, linked list 手法の根本的な問題であることが重要. 
カーネルレベルのコードでは allocation パフォーマンスが重要なため, もう一つ メモリ最適化を犠牲にしてパフォーマンスを向上させた allocator design を見る. 

## Fixed-Size Block Allocator

fixed-size block allocator では, allocator は allocation に必要な分よりも大きなメモリブロックを返すことが多く, internal fragmentation によって無駄なメモリ消費が発生する. 
一方で, 適当なメモリブロックを見つけるのに必要な時間は圧倒的に減少し, ずっと高速な allocation となる. 

### Introduction

fixed-size block allocator の基本概念は次のようなものである. リクエストされただけのメモリを allocate するのではなく, いくつかのブロックサイズを定義しておいて, 各 allocation を一つ大きいサイズへと丸めこんでしまう.
例えば, 16, 64, 512 バイトのブロックサイズにおいて, 4バイトの allocation では 16 バイトのブロックを返し, 48 バイトの allocation では 64バイトのブロックを返し, 128バイトの allocation では 512バイトのブロックを返す. 

linked list allocator に似ていて, linked list を使用していないメモリにつくることでそれらを追跡する. 
しかし, 一つのリストを使うのではなく, 各サイズに対して異なるリストをつくる. 
各リストは一つのサイズのブロックを保存する. 

一つの `head` ポインタではなく, `head_16`, `head_64`, `head_512` の 3つのポインタを保持する. 
ひとつのリストにあるすべてのノードは同じサイズである. 
例えば,  `head_16` ポインタで始まるリストは 16バイトサイズのブロックのみを保持する. 
つまり, もう各ノードにサイズ情報を保存する必要がない. 

リストの各要素は同じサイズであるため, 各要素は allocation request にとって等しく suitable である. 
つまり, 以下のステップを使うことで非常に効率的な allocation を実行可能
- 要求された allocation size を次のブロックサイズに round up する. たとえば, 12バイトの allocaiton が要求されたとき, 16バイトのブロックサイズが選択される. 
- そのリストから head pointer を取ってくる. 
- リストの最初のブロックをリストから削除して返す. 

リストの最初の要素を返せばいいので, 適当な要素を探してリストをたどる必要がなくなる. 
従って, linked list allocator よりも allocation がずっと速くなる. 

#### Block Sizes and Wasted Memory
ブロックサイズによって, rounding up によって多くのメモリを無駄にしてしまう. 
例えば 128バイトの allocation に 512バイトブロックが返されると, allocate された 3/4 のメモリが実際には使われない. 
合理的なブロックサイズを定義することで, 無駄なメモリをある程度削減することができる. 
例えば, 2の累乗数をブロックサイズとして使うと, worst case でメモリの無駄を半分にまで制限でき, average case では 1/4 の allocation size だけが無駄になる. 

通常, プログラムでのよくある allocation size に基づいてブロックサイズを最適化する. 
例えば, 24バイトの allocation を多く実行するプログラムのメモリ使用を改善するためにブロックサイズに 24を追加する可能性がある. 
この方法では, 速度を落とすことなく無駄なメモリを削減することができる可能性がある. 

#### Deallocation
allocation のように deallocation もまた高速となる. 
以下の手順で deallocation は実行される: 
- 解放された allocation size を次に大きいブロックサイズに round up する. このステップ:w
はコンパイラが `alloc` によって返されたブロックサイズではなく要求される allocation size のみしか `dealloc` に渡せないために必要となる. 同じ size-adjustment 関数を `alloc` と `dealloc` で用いることで正しい量のメモリを解放できる. 
- head pointer をリストから取ってくる. 
- 解放されたブロックを head pointer を更新することでリストの先頭に加える. 

dellocation にもリストの探索が必要にならない. 

#### Fallback Allocator
大きな allocation (> 2KB) がほとんどないと仮定すると, 特に OS kernel では, これらの allocation に異なる allocator へと fall back することが理解できるかもしれない. 
例えば, 2048バイトより大きい allocation に対して, メモリの無駄を削減するために linked list allocator へと fall back することもありえる. 
そのようなサイズの allocation はほとんどないと想定されるため, linked list は小さく (de)allociton もある程度速く行える. 

#### Creating new Blocks
上記では常にすべての allocation request を満たす特定のサイズのメモリブロックが十分存在することを仮定していた. 
しかし, どこかのタイミングで linked list が空になる可能性もある. 
このとき, 以下の 2つの方法で新しい使用されていない特定のサイズのブロックつくりだすことができる: 
- fallback allocator から新しいブロックを allocate する. 
- 他のリストのより大きなブロックを分割する. これは各ブロックサイズが 2の累乗である場合によい. 例えば, 32バイトのブロックは 2つの 16バイトのブロックへと分割可能である.

今回は fallback allocator を用いて新しいブロックの allocate を行う. 

### Implemetation

#### List Node

`src/allocator.rs`: 
```rust
pub mod fixed_size_block;
```

`src/allocator/fixed_size_block.rs`: 
```rust
struct ListNode {
    next: Option<&'static mut ListNode>
}
```

`size` のフィールドはない. 

#### Block Sizes

`src/allocator/fixed_size_block.rs`: 
```rust
// 2の累乗
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];
```

#### The Allocator Type

```rust
pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()], // 各 head の配列
    fallback_allocator: linked_list_allocator::Heap, // > 2048KB な allocation に対応する linked-list allocator. fixed-size list の要素がなくなった場合にも使用される. 
}
```

自力実装した `linked_list_allocator` を使うことも可能だが, fragmentation に対しての対策が実装されていないため外部実装を使う. 

`src/allocator/fixed_size_block.rs`: 
```rust
impl FixedSizeBlockAllocator {
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.fallback_allocator.init(heap_start, heap_size);
    }
}
```

```rust
use alloc::alloc::Layout;
use core::ptr;

impl FixedSizeBlockAllocator {
    /// Allocates using the fallback allocator.
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        match self.fallback_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(), // 存在すれば, ポインタを返す
            Err(_) => ptr::null_mut(), // 失敗した場合はヌルポインタを返す
        }
    }
}
```

#### Calculating the List Index

どのサイズのブロックを使用するかを決定する `list_index` メソッドを定義. 

```rust
fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align()); // size() と align() の最大値
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size) // next-larger block の index を返す
}
```

#### Implementing `GlobalAlloc`
`GlobalAlloc` を実装する. 
```rust
use super::Locked;
use alloc::alloc::GlobalAlloc;

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        todo!(); // TODO!
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        todo!(); // TODO!
    }
}
```

`alloc` の実装

```rust
unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    let mut allocator = self.lock();
    match list_index(&layout) {
        Some(index) => {
            match allocator.list_heads[index].take() { // リストをとってくる
                Some(node) => { // 
                    allocator.list_heads[index] = node.next.take();
                    node as *mut ListNode as *mut u8
                }
                None => { // list にブロックが存在しなければ新しいブロックを追加
                    let block_size = BLOCK_SIZES[index]; // 
                    let block_align = block_size;
                    let layout = Layout::from_size_align(block_size, block_align)
                        .unwrap();
                    allocator.fallback_alloc(layout)
                }
            }
        }
        None => allocator.fallback_alloc(layout), // 適当な block size がなければ fallback_alloc を使用する. 
    }
}
```

`dealloc` の実装

```rust
use core::{mem, ptr::NonNull};

// inside the `unsafe impl GlobalAlloc` block

unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    let mut allocator = self.lock();
    match list_index(&layout) {
        Some(index) => {
            let new_node = ListNode { // list につなげる新しい node をつくる
                next: allocator.list_heads[index].take(),
            };
            assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);  
            assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);  
            let new_node_ptr = ptr as *mut ListNode;
            new_node_ptr.write(new_node); // deallocate される ptr に node を書き込む
            allocator.list_heads[index] = Some(&mut *new_node_ptr);
        }
        None => { // 適当な block size がない場合, それは fallback allocator で allocate されたものだから, fallback allocator による deallocate を行う
            let ptr = NonNull::new(ptr).unwrap();
            allocator.fallback_allocator.deallocate(ptr, layout);
        }
    }
}
```

特筆事項: 
- block list から allocate されたブロックと fallback allocator から allocate するブロックを区別しない. つまり `alloc` で新しく作られたブロックは, `dealloc` は block list へと追加され, リストのサイズが増加する. 
- `alloc` メソッドは新しいブロックをつくり出す唯一の方法である. なので今回の実装では空の block list から始めて, allocation が実行されるときに node が追加される. 

### Using it

`FixeSizeBlockAllocator` を使用するため, `ALLOCATOR` static を書き換える. 

### Discussion
fixed-size block の手法は連結リストの手法よりもずっと効率的であるが, 2の累乗のブロックサイズを用いる場合には最大半分のメモリを無駄に消費してしまう. 
このトレードオフが価値のあるものか否かはアプリケーション次第となる. 
OS カーネルではパフォーマンスが非常に重要視され, fixed-size block の手法はベターな選択と言えるだろう. 

現在の実装からの改善点も多くある: 
- fallback allocator を用いて遅延でブロックを allocate するのではなく, 最初に要素を list に追加しておくことで初期の allocation においてよりよいぱふぉマンスが得られる. 
- 実装をより簡単にするため, ブロックサイズは 2の累乗のみであった. 別の方法で alignment を保存する (そして計算する) ことで, 任意のブロックサイズを実装できる. この方法では, メモリの無駄を最小化するためにさらに多くのブロックサイズを追加できる. e.g. よくある allocation size. 
- 現在の実装では新しいブロックは作るだけで解放しない. これによって fragmentation が発生し, 大きな allocation では allocation 失敗が発生する可能性もある. リストの最大長を強制するのが適当かもしれない. 最大長に到達したときは, deallocation 時にリストに追加するのではなく fallback allocator を用いた解放を行う (隣接の空き領域は merge されるので). 
- page allocator などで, ブロックサイズを最大 4KiB に設定し linked list allocator を完全に drop することが適当かもしれない. この利点は fragmentation を削減しパフォーマンスを改善することである. 

### Variations

fixed-size allocator には多くの亜種が存在する. 有名な例は slab allocator と buddy allocator であり, Linux 等でも使用されている. 

#### Slab Allocator

slab allocator では kernel で選択された型に直接対応したブロックサイズを使う. 
この方法では, これらの型の allocation は block size にちょうど適合するためメモリの無駄がない. 

slab allocation は他の allocator と組み合わせられることが多い. 
For example, it can be used together with a fixed-size block allocator to further split an allocated block in order to reduce memory waste. 
It is also often used to implement an object pool pattern on top of a single large allocation. 

---
TODO: slab allocator について

参考: 
<!-- - [Redox Slab Allocator で学ぶRustベアメタル環境のヒープアロケータ](https://qiita.com/tomoyuki-nakabayashi/items/e0bd16e9105163cecafb) -->
- [メモリシステム、Buddyシステム、kmalloc、スラブアロケータ](http://www.coins.tsukuba.ac.jp/~yas/coins/os2-2010/2011-01-11/)

- buddy allocator で 4KiB ごとに allocation を行う
- その 4KiB をどう割り当てるかを slab allocator が決定する
- メモリを割り当てられる構造体ごとに slab をつくり, 複数のインスタンスを保存可能にする. 
- そうすると, インスタンスが解放され再アロケートされてもキャッシュに残りやすい
- キャッシュをうまくつかうことでロックを減らして高速化

#### Buddy Allocator

解放されたブロックを管理するために連結リストを使用するのではなく, ブロックサイズが2の累乗のとき二分木を使うのが buddy allocator. 
一定のサイズの新しいブロックが必要なとき, より大きなサイズのブロックを 2つに分割し, よってツリーに二つの child node をつくり出す. ブロックが再び解放された場合, ツリー内の近隣のブロックが分析される. その近隣ブロックが同じく解放されていた場合, ふたつのブロックは結合して 2倍サイズのブロックとなる. 

このマージプロセスの利点は external fragmentation が削減されて小さい解放されたブロックも大きな allocation に再利用されること. また fallback allocator を使用しないため, 性能の予測が簡単なこと. 
この方法の最大の欠点は 2の累乗のブロックサイズのみ二しか対応していないことで, internal fragmentation が大きいこと (最悪で 50%, 平均で 25%). 
この理由から buddy allocator は slab allocator とともに使用され, allocate されたブロックをさらに小さいブロックへと分割する. 

参考: [50年前に作られたメモリ管理アルゴリズム「Buddy memory allocation」](https://codezine.jp/article/detail/9325)



---
## const について
[原文](https://varkor.github.io/blog/2019/01/11/const-types-traits-and-implementations-in-Rust.html)
- `const fn` 
- `const` trait bounds
- `const` in traits

### `const` について
> Compile-time constants and compile-time evaluable functions

コンパイル時定数とコンパイル時評価関数

### `const fn` について
`const` のもう一つの使い方が `const fn`. 
`const fn` は関数を `const` や `static` な要素の中 (const contexts) での呼び出しを可能にする. 
`const fn` は実行可能な操作を制限され, コンパイル時に評価可能であるようになる. 

### `const` と `static` について
- `const` はグローバル定数
- `const` はコンパイル時に計算が完了する必要がある. 当然不変. メモリに存在するわけではなく, コンパイル時にバイナリに埋め込まれる. 
- `static` はグローバル変数
- `static` はメモリの固定領域に配置される, `'static` lifeitme を持つ変数. `static mut` で可変にすることも可能 (変更は `unsafe`. 変更には mutex を使うのが一般的. ). 

### `const` と generics
[Const generics](https://doc.rust-lang.org/reference/items/generics.html#const-generics)

後で読む

---

## RAII について

[RAII解説 - Qiita](https://qiita.com/wx257osn2/items/e2e3bcbfdd8bd02872aa) より, 

RAII を用いたポインタ管理
```cpp
#include<memory>

int main(){
  std::cout << "ここはまだなにもない" << std::endl;
  {
    std::unique_ptr<int> ptr = new int(3);
    std::cout << "ここでメモリを確保かつ変数を初期化" << std::endl;
    std::cout << *ptr << std::endl;
  }//!Here!
  std::cout << "ptrはもう存在しない" << std::endl;
}
```

RAII を用いないポインタ管理
```cpp
int main(){
  std::cout << "ここはまだなにもない" << std::endl;
  {
    int* ptr = new int(3);
    std::cout << "ここでメモリを確保かつ変数を初期化" << std::endl;
    std::cout << *ptr << std::endl;
    delete ptr;//メモリを解放
  }//ptrを破棄
  std::cout << "ptrはもう存在しない" << std::endl;
}
```

- リソースの確保と変数の初期化
- メモリの解放と変数の破棄
を紐づける. 同時に行う. 