import requests
from dataclasses import dataclass

class Broker:

    def req_init(self):
        r = requests.get(self.path + "/init")
        return r.json()

    def req_fetch_quotes(self):
        r = requests.get(self.path + "/fetch_quotes")
        return r.json().get("quotes")

    def req_insert_order(self, order):
        r = requests.post(self.path + "/insert_order", json={"order": order})
        return r.status_code == 200

    def req_tick(self):
        r = requests.get(self.path + "/tick")
        return r.json()

    def __init__(self, path):
        self.path = path
        self.init = self.req_init()
        self.quotes = self.req_fetch_quotes()
        self.positions = {}
        self.cash_balance = 100_000
        self.has_next = True
    
    def get_position(self, symbol):
        return self.positions.get(symbol, 0)

    def fetch_quotes(self):
        return self.req_fetch_quotes()
    
    def tick(self):
        if not self.has_next:
            return

        tick = self.req_tick()

        for trade in tick["executed_trades"]:
            symbol = trade["symbol"]
            position = self.positions.get(symbol)
            if not position:
                self.positions[symbol] = trade['quantity']
            else:
                if trade["typ"] == "Buy":
                    self.positions[symbol] += trade['quantity']
                else:
                    self.positions[symbol] -= trade['quantity']
            
            if trade["typ"] == "Buy":
                self.cash_balance -= trade["value"]
            else:
                self.cash_balance += trade["value"]
        return tick["has_next"]
   
    def insert_order(self, order):
        if order["order_type"] == "MarketSell":
            symbol = order["symbol"]
            position = self.positions.get(symbol)
            if not position:
                raise ValueError("Can't sell something you don't already own")
        return self.req_insert_order(order)
