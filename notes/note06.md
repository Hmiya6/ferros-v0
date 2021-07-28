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

「呼び出すことを失敗」とはなにか? ハンドラが存在しないことか? ハンドラがスワップされたことか? ハンドラ自体が例外を発生させたのか?















