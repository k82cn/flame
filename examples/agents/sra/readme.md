# Simple Research Agent (SRA)

In the rapidly evolving landscape of AI and automation, tool-augmented agents are emerging as powerful solutions for complex task orchestration. Today, we'll explore the **Simple Research Agent (SRA)**, a compelling example built with Flame that demonstrates how an intelligent agent can leverage specialized tools to automate research workflows.

## What is the Simple Research Agent (SRA)?

The SRA is a streamlined, tool-augmented agent system designed to automate research tasks. It leverages advanced language models (DeepSeek) with LangChain/LangGraph integration and sophisticated tool orchestration to efficiently gather information from the web, process it through a vector database, and generate comprehensive research reports.

At its core, SRA embodies the principle of **intelligent tool orchestration** - a single agent coordinates multiple specialized tools and services, working together to achieve a common goal: producing high-quality research reports on any given topic.

## The Two-Component Architecture

The SRA system consists of two main components working together:

### 1. Research Agent (`sra.py`) - The Orchestrator and Writer

The Research Agent is the heart of the SRA system, combining orchestration, data collection, and report generation into a unified agent. It serves as:

- **Research Coordinator**: Interprets research topics and determines investigation scope
- **Information Gatherer**: Searches the web and manages the crawling process
- **Data Analyst**: Executes Python scripts for computational analysis
- **Report Generator**: Synthesizes collected information into comprehensive research reports

```python
sys_prompt = """
You are a writer agent for research; you will write the research paper based on the research topics and the necessary information from the tools.
As a writer, you should follow the following rules:
    1. You should write the research paper in a professional and academic style.
    2. The research paper should also include a prediction section, which should be based on the necessary information from the tools.
    3. Try to use python script to do the calculation and prediction.
    4. You should use the necessary information from the vector database to write the research paper.
    5. The research paper should be written in a concise and clear manner.
    6. The research paper should be written in a logical and coherent manner.
    7. The research paper should be written in a consistent manner.
    8. The research paper should be written in a markdown format.
"""
```

The agent is built using LangChain and LangGraph, equipped with three powerful tools:

#### Tool 1: `web_search` - Web Discovery and Crawling
Searches the web using DuckDuckGo and orchestrates parallel crawling operations:
- Searches for recent news and articles (up to 20 results per topic)
- Focuses on current information (filtered by day)
- Invokes the crawler service asynchronously for each URL
- Tracks crawling progress with a `Counter` class that monitors successful, failed, and error states
- Returns the number of successfully crawled URLs

```python
@tool
def web_search(topics: list[str]) -> int:
    """
    Search the web for the topics and persist the content of the web page to the vector database.
    Return the number of urls crawled successfully.
    """
    wrapper = DuckDuckGoSearchAPIWrapper(time="d", max_results=20)
    search = DuckDuckGoSearchResults(api_wrapper=wrapper, source="news", output_format="list")

    counter = Counter()
    tasks = []
    for topic in topics:
        items = search.invoke(topic)
        for item in items:
            task = web_crawler.invoke(WebPage(url=item["link"]), informer=counter)
            tasks.append(task)

    await asyncio.gather(*tasks)
    return counter.succeed
```

#### Tool 2: `collect_data` - Vector Database Retrieval
Queries the vector database using semantic search:
- Embeds the search topic using the embedding service
- Retrieves the most relevant information chunks (top 3 results)
- Leverages cosine similarity for intelligent content matching
- Returns content payloads with URLs and text chunks

#### Tool 3: `run_script` - Computational Analysis
Integrates with `flmexec` for secure Python script execution:
- Supports PEP 723 inline script metadata for dependency declaration
- Executes code in a sandboxed environment via `uv run`
- Enables data analysis, calculations, and predictions
- Returns script stdout for incorporation into reports

```python
@tool
def run_script(code: str) -> str:
    """
    Run the python script and return the result. The stdout of the script will be returned as a string.
    The script will be launched by `uv run` command with the dependencies declared in the script.
    For example, if the script depends on `numpy`, you should declare the dependencies in the script like this:
    ```
    # /// script
    # dependencies = [
    #   "numpy",
    # ]
    # ///
    ```
    """
    script_runner = await flamepy.create_session("flmexec")
    output = await script_runner.invoke(Script(language="python", code=code))
    return output.decode("utf-8")
```

### 2. Crawler Service (`crawler.py`) - The Web Content Processor

The Crawler Service is a Flame application that processes individual web pages:
- Downloads web page content using proper headers (identifies as "Xflops Crawler")
- Converts HTML to clean markdown using MarkItDown
- Chunks content into manageable pieces (1024 bytes per chunk)
- Generates embeddings for each chunk via the embedding service
- Stores chunks with metadata (URL, chunk index, content) in Qdrant vector database

```python
@ins.entrypoint
def crawler(wp: WebPage) -> Answer:
    text = requests.get(wp.url, headers=headers).text
    
    md = markitdown.MarkItDown()
    result = md.convert(stream).text_content
    
    chunk_size = min(1024, len(result))
    
    for chunk in range(0, len(result), chunk_size):
        vector = embedding_client.embed(result[chunk:chunk+chunk_size])
        client.upsert(collection_name="sra", points=[
            PointStruct(id=f"{uuid.uuid4()}", vector=vector,
                       payload={"url": wp.url, "chunk": chunk, "content": result[chunk:chunk+chunk_size]})
        ])
    
    return Answer(answer=f"Crawled {wp.url}")
```

## Deployment Configuration

The SRA system is deployed using Flame's application manifest format (`sra.yaml`):

```yaml
---
metadata:
  name: crawler
spec:
  working_directory: /opt/examples/agents/sra/
  tags:
    - Tool
  environments:
    SILICONFLOW_API_KEY: sk-xxxxxxxxxxxxxxxxx
  command: /usr/bin/uv
  arguments:
    - run
    - -n
    - crawler.py
    - apis.py

---
metadata:
  name: sra
spec:
  working_directory: /opt/examples/agents/sra/
  max_instances: 1
  tags:
    - Agent
  environments:
    DEEPSEEK_API_KEY: sk-xxxxxxxxxxxxxxxxx
    SILICONFLOW_API_KEY: sk-xxxxxxxxxxxxxxxxx
  command: /usr/bin/uv
  arguments:
    - run
    - -n
    - sra.py
    - apis.py
```

Key configuration details:
- **Crawler**: Deployed as a Tool, can scale to multiple instances for parallel processing
- **SRA Agent**: Limited to 1 instance (`max_instances: 1`) to maintain consistency
- **Environment Variables**: Requires API keys for DeepSeek (LLM) and SiliconFlow (embeddings)
- **Runtime**: Uses `uv run` for dependency management and execution

## Project Dependencies

The SRA system uses modern Python tooling with `uv` for dependency management (`pyproject.toml`):

```toml
[project]
name = "sra"
version = "0.1.0"
requires-python = ">=3.12"
dependencies = [
  "flamepy",              # Flame SDK for agent framework
  "ddgs",                 # DuckDuckGo search
  "langchain",            # LangChain framework
  "langgraph",            # LangGraph for agent orchestration
  "langchain-deepseek",   # DeepSeek LLM integration
  "langchain-community",  # Community tools and utilities
  "qdrant-client>=1.14.1", # Vector database client
  "requests>=2.32.3",     # HTTP requests
  "markitdown",           # HTML to Markdown conversion
  "python-dotenv",        # Environment variable management
  "pytest",               # Testing framework
]
```

## The Supporting Infrastructure

### Vector Database Integration
The system uses Qdrant as its vector database, configured with:
- 1024-dimensional vectors using cosine similarity
- Automatic collection creation and management (collection name: "sra")
- Efficient semantic search capabilities
- Stores content chunks with metadata (URL, chunk index, content)

### Embedding Service (`embed.py`)
The EmbeddingClient provides text-to-vector conversion using:
- **Provider**: SiliconFlow API (api.siliconflow.cn)
- **Model**: Qwen/Qwen3-Embedding-0.6B
- **Dimensions**: 1024-dimensional embeddings
- **Format**: Float encoding for precise semantic matching

```python
class EmbeddingClient:
    def __init__(self, api_key: str = None, model: str = "Qwen/Qwen3-Embedding-0.6B"):
        self.api_url = "https://api.siliconflow.cn/v1/embeddings"
        # ... initialization
    
    def embed(self, text: str) -> list[float]:
        payload = {
            "model": self.model,
            "input": text,
            "encoding_format": "float",
            "dimensions": 1024
        }
        # Returns 1024-dimensional vector
```

### Security and Isolation
The `flmexec` application provides secure script execution:
- Sandboxed environment for Python scripts
- Uses `uv run` for dependency management
- Supports PEP 723 inline script metadata
- Isolated from the main system to ensure safety

### Session Management
The system leverages Flame's session management for efficient resource utilization:
- Reuses `script_runner` session across multiple script executions
- Reuses `web_crawler` session for parallel crawling operations
- Async operations with `asyncio.gather` for parallel processing

## How to Use the SRA

### Client Usage

The SRA system is invoked through a simple client interface (`client.py`):

```python
import flamepy
import asyncio
from apis import Question, Answer

async def build_research_report():
    # Create a session to the SRA agent
    sra = await flamepy.create_session("sra")

    # Invoke the agent with a research topic
    output = await sra.invoke(Question(topic="Write a report about 2025 Nvidia stock performance"))
    
    # Parse and display the answer
    answer = Answer.from_json(output)
    print(answer.answer)
   
    # Clean up the session
    await sra.close()

if __name__ == "__main__":
    asyncio.run(build_research_report())
```

The client demonstrates:
- **Session Creation**: Establishes a connection to the deployed SRA agent
- **Request/Response Model**: Uses typed `Question` and `Answer` objects
- **Async Operations**: Leverages Python's asyncio for efficient execution
- **Resource Management**: Properly closes sessions after use

### Running the System

1. **Deploy the applications** using Flame:
   ```bash
   flmctl create -f sra.yaml
   ```

2. **Ensure infrastructure is running**:
   - Qdrant vector database (accessible at `qdrant:6333`)
   - SiliconFlow API access for embeddings
   - DeepSeek API access for LLM
   - `flmexec` service for script execution

3. **Run the client**:
   ```bash
   python client.py
   ```

## The Workflow in Action

Here's how the SRA system operates when given a research topic:

1. **User Input**: User provides a research topic to the SRA agent via `client.py`
   ```python
   sra = await flamepy.create_session("sra")
   output = await sra.invoke(Question(topic="Write a report about 2025 Nvidia stock performance"))
   ```

2. **Agent Planning**: The agent analyzes the topic and determines which tools to use

3. **Web Search and Crawling** (via `web_search` tool):
   - Searches DuckDuckGo for relevant recent news articles (up to 20 per topic)
   - Creates parallel crawling tasks for all discovered URLs
   - Each URL is processed by the crawler service asynchronously
   - Crawler downloads content, converts to markdown, chunks it, and embeds it
   - All chunks are stored in the Qdrant vector database with metadata
   - Returns count of successfully crawled URLs

4. **Data Collection** (via `collect_data` tool):
   - Agent embeds the research topic to create a query vector
   - Performs semantic search in Qdrant (top 3 most relevant chunks)
   - Retrieves content chunks with source URLs

5. **Computational Analysis** (via `run_script` tool, optional):
   - Agent generates Python scripts for data analysis or predictions
   - Scripts execute in isolated `flmexec` environment
   - Results are incorporated into the research report

6. **Report Generation**:
   - Agent synthesizes all collected information
   - Produces a comprehensive markdown report following academic style guidelines
   - Includes predictions based on data and computational analysis

7. **Delivery**: Final report is returned to the user as an `Answer` object

## Conclusion

The Simple Research Agent showcases the power of tool-augmented AI agents in automating complex, multi-step workflows. By combining a sophisticated LangChain agent with specialized tools and services, Flame enables the creation of intelligent systems that can tackle real-world research tasks with efficiency and reliability.

Key architectural highlights:
- **Unified Agent**: Single research agent with multiple specialized tools (web search, data collection, script execution)
- **Parallel Processing**: Asynchronous crawling operations for efficient data collection
- **Semantic Search**: Vector database integration for intelligent information retrieval
- **Computational Capabilities**: Secure script execution for data analysis and predictions
- **Modular Design**: Clean separation between agent logic and crawler service

Whether you're building research tools, content generation systems, or any application requiring AI-powered automation, the SRA example provides a solid foundation for understanding how to architect and implement tool-augmented agent solutions with Flame.

The future of AI lies in intelligent agents that can orchestrate complex workflows using specialized tools - and the SRA is a perfect example of this paradigm in action.

---

*Ready to build your own tool-augmented agent system? Explore the SRA example in the Flame repository and start creating intelligent, automated research workflows today.*
