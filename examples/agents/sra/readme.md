# Simple Research Agent (SRA)

In the rapidly evolving landscape of AI and automation, multi-agent systems are emerging as powerful tools for complex task orchestration. Today, we'll explore the **Simple Research Agent (SRA)**, a compelling example built with Flame that demonstrates how multiple specialized agents can collaborate to automate research workflows.

## What is the Simple Research Agent (SRA)?

The SRA is a streamlined, modular multi-agent system designed to automate research tasks. It leverages advanced language models and sophisticated tool integration to efficiently gather information from the web, process it through a vector database, and generate comprehensive research reports.

At its core, SRA embodies the principle of **separation of concerns** - each agent has a specific responsibility, working together to achieve a common goal: producing high-quality research reports on any given topic.

## The Three-Agent Architecture

The SRA system consists of three specialized agents, each with distinct roles and capabilities:

### 1. Supervisor Agent (`sra.py`) - The Orchestrator

The Supervisor Agent serves as the frontend and coordinator of the entire SRA system. Think of it as the project manager who:

- **Understands user requirements**: Interprets research topics and determines the scope of investigation
- **Creates execution plans**: Organizes the workflow between collector and writer agents
- **Coordinates operations**: Manages the sequence of data collection and report generation
- **Provides user interface**: Acts as the primary entry point for user interactions

```python
sys_prompt = """
You are the entrypoint of SRA (Simple Research Agent) which is a multi-agent system.
You will try to understand the research topic from the user and organize collector and writer agents to build the report.
As the supervisor of SRA, you should follow the following rules:
    1. You should understand the research topic from the user.
    2. You should collect the necessary information based on the understanding of the user's topic.
    3. You should build a plan to organize the collector and writer agents to build the report.
    4. By default, it should be a research report of this year.
"""
```

The supervisor uses LangGraph's ReAct agent pattern, equipped with two powerful tools:
- `data_collector_agent`: Delegates data gathering to the collector agent
- `report_writer_agent`: Orchestrates report generation through the writer agent

### 2. Collector Agent (`collector.py`) - The Information Gatherer

The Collector Agent is responsible for the critical task of gathering relevant information from the internet. Its workflow is sophisticated and multi-step:

#### Search and Discovery
Using DuckDuckGo's search API, the collector:
- Searches for recent news and articles related to the research topic
- Retrieves up to 20 relevant links with time-filtered results
- Focuses on current information (filtered by day)

#### Web Crawling and Processing
The collector then leverages a specialized crawler application (`crawler.py`) that:
- Downloads web page content using proper headers
- Converts HTML to clean markdown using MarkItDown
- Chunks content into manageable pieces (8KB chunks)
- Generates embeddings for semantic search
- Stores everything in a Qdrant vector database

```python
@tool
def search_topics(topic: str) -> list[str]:
    """
    Search the topic on the internet by DuckDuckGo and return the list of links found about the topic.
    """
    wrapper = DuckDuckGoSearchAPIWrapper(time="d", max_results=20)
    search = DuckDuckGoSearchResults(api_wrapper=wrapper, source="news", output_format="list")
    items = search.invoke(topic)
    return [item["link"] for item in items]
```

The collector also implements a sophisticated task monitoring system with a `Counter` class that tracks successful and failed crawling operations, providing real-time feedback on the data collection process.

### 3. Writer Agent (`writer.py`) - The Report Generator

The Writer Agent is the final piece of the puzzle, responsible for synthesizing collected information into comprehensive research reports. Its capabilities include:

#### Data Retrieval
- Queries the vector database using semantic search
- Retrieves the most relevant information chunks based on the research topic
- Leverages embeddings for intelligent content matching

#### Report Generation
The writer follows strict guidelines for producing professional research reports:
- Academic and professional writing style
- Logical and coherent structure
- Consistent formatting in markdown
- Includes prediction sections based on collected data

#### Computational Analysis
One of the most powerful features is the integration with `flmexec` - a secure script execution environment that allows the writer to:
- Generate Python scripts for data analysis and calculations
- Execute code in a sandboxed environment
- Incorporate computational results into the final report

```python
async def run_script_async(code: str) -> str:
    script_runner = await flamepy.create_session("flmexec")

    output = await script_runner.invoke(Script(language="python", code=code))

    await script_runner.close()

    return output.decode("utf-8")

@tool
def run_script(code: str) -> str:
    """
    Run the python script and return the result.
    The script will be launched by `uv run` command with the dependencies declared in the script.
    """
    return asyncio.run(run_script_async(code))
```

## The Supporting Infrastructure

### Vector Database Integration
The system uses Qdrant as its vector database, configured with:
- 2560-dimensional vectors using cosine similarity
- Automatic collection creation and management
- Efficient semantic search capabilities

### Embedding Service
A dedicated embedding API service handles text-to-vector conversion, enabling semantic search across all collected content.

### Security and Isolation
The `flmexec` application provides secure script execution, ensuring that any computational analysis runs in an isolated environment without affecting the main system.

## The Workflow in Action

Here's how the SRA system operates when given a research topic:

1. **User Input**: User provides a research topic to the Supervisor Agent
2. **Planning**: Supervisor analyzes the topic and creates an execution plan
3. **Data Collection**: 
   - Collector searches for relevant links using DuckDuckGo
   - Crawler downloads and processes web pages
   - Content is embedded and stored in the vector database
4. **Report Generation**:
   - Writer retrieves relevant information from the vector database
   - Generates analysis and predictions using Python scripts if needed
   - Produces a comprehensive markdown report
5. **Delivery**: Final report is returned to the user

## Conclusion

The Simple Research Agent showcases the power of multi-agent systems in automating complex, multi-step workflows. By combining specialized agents with robust infrastructure, Flame enables the creation of sophisticated AI systems that can tackle real-world research tasks with efficiency and reliability.

Whether you're building research tools, content generation systems, or any application requiring coordinated AI agents, the SRA example provides a solid foundation for understanding how to architect and implement multi-agent solutions with Flame.

The future of AI lies not in single, monolithic models, but in coordinated systems of specialized agents working together - and the SRA is a perfect example of this paradigm in action.

---

*Ready to build your own multi-agent system? Explore the SRA example in the Flame repository and start creating intelligent, coordinated AI workflows today.*
