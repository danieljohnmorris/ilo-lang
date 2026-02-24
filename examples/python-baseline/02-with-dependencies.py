from validation import validate_address
from shipping import calculate_shipping
from pricing import apply_discount

def process_order(order: OrderRecord) -> Result[OrderRecord, str]:
    if not validate_address(order.shipping_address):
        return Err("Invalid shipping address")
    shipping = calculate_shipping(
        weight=order.total_weight,
        destination=order.shipping_address.country
    )
    discount = apply_discount(
        subtotal=order.subtotal,
        code=order.discount_code
    )
    final_total = order.subtotal - discount + shipping
    return Ok(dataclasses.replace(order, total=final_total, shipping_cost=shipping))
