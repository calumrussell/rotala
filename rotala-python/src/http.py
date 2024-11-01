import json
import requests


class HttpClient:
    def __init__(self, base_url):
        self.base_url = base_url
        self.backtest_id = None
        return

    def init(self, dataset_name):
        r = requests.get(f"{self.base_url}/init/{dataset_name}")
        json_response = r.json()
        self.backtest_id = int(json_response["backtest_id"])
        return json_response

    def tick(self):
        if self.backtest_id is None:
            raise ValueError("Called before init")

        r = requests.get(f"{self.base_url}/backtest/{self.backtest_id}/tick")
        return r.json()

    def insert_order(self, order):
        if self.backtest_id is None:
            raise ValueError("Called before init")

        val = f'{{"order": {order.serialize()}}}'
        r = requests.post(
            f"{self.base_url}/backtest/{self.backtest_id}/insert_order",
            data=val,
            headers={"Content-type": "application/json"},
        )
        return r.json()

    def modify_order(self, order_id, quantity_change):
        if self.backtest_id is None:
            raise ValueError("Called before init")

        val = f'{{"order_id": {order_id}, "quantity_change": {quantity_change}'
        r = requests.post(
            f"{self.base_url}/backtest/{self.backtest_id}/modify_order",
            data=val,
            headers={"Content-type": "application/json"},
        )
        return r.json()

    def cancel_order(self, order_id):
        if self.backtest_id is None:
            raise ValueError("Called before init")

        val = f'{{"order_id": {order_id}'
        r = requests.post(
            f"{self.base_url}/backtest/{self.backtest_id}/cancel_order",
            data=val,
            headers={"Content-type": "application/json"},
        )
        return r.json()

    def fetch_quotes(self):
        if self.backtest_id is None:
            raise ValueError("Called before init")

        r = requests.get(f"{self.base_url}/backtest/{self.backtest_id}/fetch_quotes")
        return r.json()

    def fetch_depth(self):
        if self.backtest_id is None:
            raise ValueError("Called before init")

        r = requests.get(f"{self.base_url}/backtest/{self.backtest_id}/fetch_depth")
        return r.json()

    def info(self):
        if self.backtest_id is None:
            raise ValueError("Called before init")

        r = requests.get(f"{self.base_url}/backtest/{self.backtest_id}/info")
        return r.json()

    def now(self, backtest_id):
        if self.backtest_id is None:
            raise ValueError("Called before init")

        r = requests.get(f"{self.base_url}/backtest/{self.backtest_id}/now")
        return r.json()
