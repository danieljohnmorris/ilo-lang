async def checkout(payment_id: str, amount: float, items: List[ItemLine]) -> Result[CheckoutResult, str]:
    reserved = await reserve_inventory(items)
    if isinstance(reserved, Err):
        return Err(f"Inventory unavailable: {reserved.error}")
    reservation_id = reserved.value

    charged = await charge_payment(payment_id, amount)
    if isinstance(charged, Err):
        await release_inventory(reservation_id)
        return Err(f"Payment failed: {charged.error}")
    charge_id = charged.value

    order_id = generate_order_id()
    return Ok(CheckoutResult(
        order_id=order_id,
        charge_id=charge_id,
        reservation_id=reservation_id
    ))
