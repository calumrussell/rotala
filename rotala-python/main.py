import logging

from src.broker import BrokerBuilder, Order, OrderType
from src.http import HttpClient


def get_best_and_mid(depth):
    bids = depth["SOL"]["bids"]
    asks = depth["SOL"]["asks"]

    bid_levels = [b["price"] for b in bids]
    ask_levels = [a["price"] for a in asks]

    best_bid = bid_levels[0]
    best_ask = ask_levels[0]
    mid_price = (best_bid + best_ask) / 2
    return best_bid, best_ask, mid_price


def create_grid(depth):
    best_bid, best_ask, mid_price = get_best_and_mid(depth)
    gap = round(mid_price * 0.0005, 2)

    bid_order_levels = [round(best_bid - (i * gap), 2) for i in range(1, 5)]
    ask_order_levels = [round(best_ask + (i * gap), 2) for i in range(1, 5)]
    return bid_order_levels, ask_order_levels


def risk_management(unexecuted_orders, total_value):
    gross_value = 0
    for order_id in unexecuted_orders:
        order = unexecuted_orders[order_id]
        if order.price:
            gross_value += order.qty * order.price

    if gross_value > total_value * 0.1:
        return False
    return True


def create_orders(bid_grid, ask_grid):
    orders = []
    for level in bid_grid:
        order = Order(OrderType.LimitBuy, "SOL", 10, level, None)
        orders.append(order)

    for level in ask_grid:
        order = Order(OrderType.LimitSell, "SOL", 10, level, None)
        orders.append(order)
    return orders


if __name__ == "__main__":
    logging.basicConfig(level=logging.CRITICAL)

    dataset_name = "Test"
    frequency = 250
    sim_length = frequency * 10000
    http_client = HttpClient("http://127.0.0.1:3000")
    dataset_info = http_client.dataset_info(dataset_name)
    start_date = dataset_info["start_date"]

    builder = BrokerBuilder()
    builder.init_dataset_name(dataset_name)
    builder.init_cash(100000)
    builder.init_http(http_client)
    #Clear the first date so we have quotes always
    builder.init_dates(start_date+frequency, start_date+frequency+sim_length)
    builder.init_frequency(frequency)
    brkr = builder.build()

    last_mid = -1
    while True:
        brkr.tick()

        depth = brkr.latest_depth
        if not depth:
            continue

        bid_grid, ask_grid = create_grid(depth)

        best_bid, best_ask, mid_price = get_best_and_mid(depth)
        if last_mid == -1:
            last_mid = mid_price

        mid_change = round(abs(last_mid - mid_price), 2)
        last_mid = mid_price

        risk = risk_management(brkr.unexecuted_orders, brkr.get_current_value())
        if len(brkr.unexecuted_orders) == 0:
            [brkr.insert_order(order) for order in create_orders(bid_grid, ask_grid)]
        else:
            if mid_change > 0.1:
                # In practice, we want to look for overlapping levels so we don't need
                # to clear whole book
                for order_id in brkr.unexecuted_orders:
                    order = brkr.unexecuted_orders[order_id]
                    if order.is_transaction():
                        brkr.insert_order(
                            Order(OrderType.Cancel, "SOL", 0, None, order_id)
                        )

                [
                    brkr.insert_order(order)
                    for order in create_orders(bid_grid, ask_grid)
                ]
