Binary reduction steps
======================

Implement this leaf function in `reduction_steps.s`:

    reduction_steps(value) -> steps

The non-negative integer argument arrives in `a0`. Repeatedly reduce the value using these rules:

*   When the value is odd, subtract one.
*   When the value is even, divide it by two.

Return the number of operations required to reach zero in `a0`.

For example, reducing 6 takes four operations:

    6 -> 3 -> 2 -> 1 -> 0

Translate this Python function into assembly:

```python
def reduction_steps(value: int) -> int:
    steps = 0
    while value != 0:
        if value & 1 != 0:
            value -= 1
        else:
            value >>= 1
        steps += 1
    return steps
```

Hint: test whether a value is odd by checking if its least-significant-bit: 0 means even, 1 means odd.

The python `>>=` operator shifts a value right, the equivalent of `srl` or `srli` in risc-v assembly.
