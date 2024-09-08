import requests
import json
from functools import wraps


class HttpClient:
    def __init__(self, base_url):
        self.base_url = base_url
        return

    def tick(self, backtest_id):
        r = requests.get(f"{self.base_url}/backtest/{backtest_id}/tick")
        return r.json()

    def insert_order(self, order, backtest_id):
        r = requests.post(
            f"{self.base_url}/backtest/{backtest_id}/insert_order",
            data={"order": order},
        )
        return r.json()

    def fetch_quotes(self, backtest_id):
        r = requests.get(f"{self.base_url}/backtest/{backtest_id}/fetch_quotes")
        return r.json()

    def init(self, dataset_name):
        r = requests.get(f"{self.base_url}/init/{dataset_name}")
        return r.json()

    def info(self, backtest_id):
        r = requests.get(f"{self.base_url}/backtest/{backtest_id}/info")
        return r.json()

    def now(self, backtest_id):
        r = requests.get(f"{self.base_url}/backtest/{backtest_id}/now")
        return r.json()


def json_response(inner_func):
    @wraps(inner_func)
    def wrapper(*args, **kwargs):
        return json.dumps(inner_func(*args, **kwargs))

    return wrapper


class TestHttpClient:
    def __init__(self):
        pass

    @json_response
    def tick(self, backtest_id):
        return {
            "has_next": False,
            "executed_trades": [],
            "insert_orders": [],
        }

    @json_response
    def insert_order(self, order, backtest_id):
        return {}

    @json_response
    def fetch_quotes(self, backtest_id):
        return {
            "quotes": [],
        }

    @json_response
    def init(self, dataset_name):
        return {"backtest_id": 0}

    @json_response
    def info(self, backtest_id):
        return {
            "version": "",
            "dataset": "",
        }

    @json_response
    def now(self, backtest_id):
        return {"now": 0, "has_next": True}
