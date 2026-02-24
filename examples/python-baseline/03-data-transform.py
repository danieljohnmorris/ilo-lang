from dataclasses import dataclass
from typing import List

@dataclass
class CustomerRecord:
    name: str
    email: str
    tier: str
    total_spent: float

@dataclass
class LoyaltySummary:
    customer_name: str
    level: str
    discount_percent: int

def classify_loyalty(spent: float) -> str:
    if spent >= 1000:
        return "gold"
    elif spent >= 500:
        return "silver"
    else:
        return "bronze"

def build_loyalty_summaries(customers: List[CustomerRecord]) -> List[LoyaltySummary]:
    results = []
    for c in customers:
        level = classify_loyalty(c.total_spent)
        discount = {"gold": 20, "silver": 10, "bronze": 5}.get(level, 0)
        results.append(LoyaltySummary(
            customer_name=c.name,
            level=level,
            discount_percent=discount
        ))
    return results
