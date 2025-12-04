
import flamepy
import asyncio
from apis import Question, Answer

async def build_research_report():
    sra = await flamepy.create_session("sra")

    output = await sra.invoke(Question(topic="Write a report about 2025 Nvidia stock performance"))
    answer = Answer.from_json(output)
    print(answer.answer)
   
    await sra.close()

if __name__ == "__main__":
    asyncio.run(build_research_report())
