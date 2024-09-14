import requests
import json
from functools import wraps


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

        r = requests.post(
            f"{self.base_url}/backtest/{self.backtest_id}/insert_order",
            data={"order": order},
        )
        return r.json()

    def fetch_quotes(self):
        if self.backtest_id is None:
            raise ValueError("Called before init")

        r = requests.get(f"{self.base_url}/backtest/{self.backtest_id}/fetch_quotes")
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
