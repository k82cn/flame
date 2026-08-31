from engine import DecodeEngine, PrefillEngine
from flamepy.runner import Runner


def main():
    with Runner("vllm-example") as rr:
        prefill = rr.service(PrefillEngine())
        decode = rr.service(DecodeEngine())

        out = prefill.prefill("Once upon a time")
        text = decode.decode(out, 16).get()
        print(text)


if __name__ == "__main__":
    main()
