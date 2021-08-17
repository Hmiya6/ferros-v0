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

QUESTION: 全体的によくわからない. 

物理メモリ容量が小さい機器に対しては, アクセスされるときの**一時的にのみページテーブルフレームをマップする**こともできる. 
一時マッピングをつくるのに必要なのは一つの identity-mapped level 1 table のみ


> The level 1 table in this graphic controls the first 2 MiB of the virtual address space. This is bexause it is reachable by starting at the CR3 register and following the 0th entry in the level 4, level 3, and level 2 page tables. The entry with index `8` maps the virtual page at address `32 KiB` to the physical frame at address `32 KiB`, theby identity mapping the level 1 table itself. The graphic shows this identity-mapping by the horizontal arrow at `32 KiB`. 

図中の level 1 テーブルは最初の仮想アドレス空間の 2MiB をコントロールする. 
これは, CR3 レジスタから level 4 -> 3 -> 2 とつながっており到達可能なため. 
index `8` のエントリは ``

### Recursive Page Tables

別のアプローチは, 追加のページテーブルを必要とせず, **ページテーブルを再帰的にマップする**こと. 
このアプローチの背後には level 4 ページテーブルのいくつかのエントリを level 4 ページテーブル自身にマップする考えがある. 

