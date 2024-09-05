from src.http import Http


class Broker:
    def __init__(self, base_url):
        self.http = Http(base_url)
