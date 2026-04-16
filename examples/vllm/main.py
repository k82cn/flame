from vllm import LLM

def main():
    print("Hello from vllm!")

    # Initialize the vLLM engine.
    llm = LLM(model="facebook/opt-125m")

if __name__ == "__main__":
    main()
