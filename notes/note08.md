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
その当時, CPU は 16bit アドレスを使用しており, adressable memory の量は最大 64KiB だった. これを増加させるため, 追加で segment registers が導入された, 各々は offset address を保持した. 
















