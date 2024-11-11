import argparse
import os
import boto3
from botocore import UNSIGNED
from botocore.config import Config
import lz4framed
import datetime


def path_builder(date, hour, coin):
    return f"market_data/{date}/{hour}/l2Book/{coin}.lz4"


def parse_date(string):
    return (string[0:4], string[4:6])


def zero_padding(number):
    if number < 10:
        return "0" + str(number)
    return str(number)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        prog="HL Data Fetcher",
        description="Downloads data from HL, unzips and places into directory",
    )

    parser.add_argument("-o", "--outdir")
    parser.add_argument("-c", "--coin")
    parser.add_argument("-s", "--start")

    args = parser.parse_args()

    max_year = 2024
    hours = list(range(0, 24))
    days = list(range(1, 32))
    months = list(range(1, 13))
    client = boto3.client("s3", config=Config(signature_version=UNSIGNED))
    bucket_name = "hyperliquid-archive"
    now = datetime.datetime.now()

    os.makedirs(f"{args.outdir}/{args.coin}", exist_ok=True)

    (start_year, start_month) = parse_date(args.start)
    for year in range(int(start_year), max_year + 1):
        iter_start = int(start_month) if int(year) == int(start_year) else 1

        for month in range(iter_start, 13):
            chunks = []
            file_path = f"{args.outdir}/{args.coin}/{month}"
            if os.path.exists(file_path):
                continue

            for day in days:
                try:
                    then = datetime.datetime(year, month, day)
                except ValueError:
                    # Occurs if date isn't valid
                    continue

                if then > now:
                    print("Reached the present")
                    print(file_path)
                    with open(f"{file_path}", "w") as f:
                        for chunk in chunks:
                            f.write(chunk)
                    exit(1)

                for hour in hours:
                    date_string = str(year) + zero_padding(month) + zero_padding(day)
                    key = path_builder(date_string, hour, args.coin)

                    try:
                        response = client.get_object(
                            Bucket=bucket_name,
                            Key=key,
                        )
                        contents = response["Body"].read()
                    except Exception:
                        print(f"Didn't find - {key}")
                        continue

                    uncompressed = lz4framed.decompress(contents)
                    print(f"Took - {key}")
                    chunks.append(uncompressed.decode("utf-8"))

            print(file_path)
            with open(f"{file_path}", "w") as f:
                for chunk in chunks:
                    f.write(chunk)
