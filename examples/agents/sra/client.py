import flamepy
import asyncio
from datetime import datetime

from apis import Question, Answer

async def build_research_report():
    sra = await flamepy.create_session("sra")

    topic="Write a report about 2025 Nvidia stock performance and predict the stock price in 2026"

    print(f"Building research report for topic: {topic}")

    output = await sra.invoke(Question(topic=topic))
    answer = Answer.from_json(output)

    report_timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
    report_name = f"sra_report_{report_timestamp}.md"

    with open(report_name, "w") as f:
        f.write(answer.answer)

    print(f"Research report was saved to {report_name}")

    await sra.close()

if __name__ == "__main__":
    asyncio.run(build_research_report())
