
[Allocator Designs](https://os.phil-opp.com/allocator-designs/)

# Allocator Designs のメモ

## Design Goals

allocator の役割は使用可能なヒープメモリを管理することにある. allocator は `alloc` で使用されていないメモリを返し, `dealloc` で解放されるメモリを追跡することで再使用可能にする必要がある. 
さらに重要なのは, 使用中のメモリに手を出さないこと. 

メモリ管理の正確さ以外にも, 副次的な多くの設計目標がある. 
たとえば, allocator は効率的に使用可能なメモリを使用し, fragmentation を小さくするべきである. 
さらに, 並行処理にも上手く対応し, 搭載されるプロセッサの数にもスケールするべきである. 
パフォーマンスを最大化するためには, メモリレイアウトを CPU キャッシュの観点から最適化し cache locality を向上しや false sharing を避けるようになる. 

TODO: キャッシュについてあまり知らない (CPU キャッシュ, メモリアドレス対応関係のキャッシュ, etc..) 

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

TODO: const 関数にはどんな特徴がある? 使い時はいつ?

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

TODO: `*mut u8` で生ポインタを作成しているが `*mut u64` でなくてもいいのか

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

TODO: なぜだめなのか

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
            ptr::null_mut() // out of memory
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
(前回のミーティング後に整理し忘れていた)
