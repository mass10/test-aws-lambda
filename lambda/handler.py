import time
import json


def handler(event, context):
    duration = event.get("sleep", 3)
    time.sleep(duration)
    return {
        "statusCode": 200,
        "body": json.dumps({
            "message": "ok",
            "request_id": context.aws_request_id,
            "sleep": duration,
        }),
    }
