def total(price: float, quantity: int, rate: float) -> float:
    sub = price * quantity
    tax = sub * rate
    return sub + tax

assert total(10, 2, 0.2) == 24
assert total(100, 1, 0) == 100
