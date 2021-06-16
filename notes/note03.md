[VGA Text Mode](https://os.phil-opp.com/vga-text-mode/)

# VGA Text Mode の一次メモ

VGA テキストモードは文字をスクリーンにプリントする簡単な方法.

## The VGA Text Buffer

典型的な VGA テキストバッファは 25行x80列の 2次元配列.

VGA text buffer's format:
```
bits: value
0-7: ASCII code point
8-11: Foreground color
12-14: Background color
15: Blink
```
Color



##

##
