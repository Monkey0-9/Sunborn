# Sunborn Universal AI Agent Integration & Quickstart Guide

This guide explains how to connect any AI agent or framework to the **Sunborn Semantic AI Gateway**. Because Sunborn provides a standard OpenAI-compatible API (`/v1/chat/completions`) alongside native Agent DAG execution memoization, any agent in any language can achieve instant execution reuse without rewriting application logic.

---

## 1. Quickstart: OpenAI Python SDK (Drop-In)

Simply configure `base_url` pointing to the Sunborn Gateway:

```python
from openai import OpenAI

# 1-line configuration: Point OpenAI client to Sunborn
client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="key-enterprise-fintech"
)

response = client.chat.completions.create(
    model="gpt-4o",
    messages=[
        {"role": "system", "content": "You are a financial research analyst."},
        {"role": "user", "content": "Calculate the debt-to-equity ratio for Acme Corp in FY2024."}
    ],
    temperature=0.2
)

print(response.choices[0].message.content)

# Inspect Sunborn Execution-Reuse Metadata:
# Headers / response include:
#   sunborn_cached: True / False
#   sunborn_tier: "c1_exact" / "c2_semantic" / "origin"
```

---

## 2. LangChain & LangGraph Integration

In LangChain, set `openai_api_base` or use the standard `ChatOpenAI` client:

```python
from langchain_openai import ChatOpenAI
from langchain_core.messages import HumanMessage, SystemMessage

llm = ChatOpenAI(
    model="gpt-4o",
    openai_api_base="http://localhost:8080/v1",
    openai_api_key="key-enterprise-fintech",
    temperature=0.0
)

messages = [
    SystemMessage(content="You are an autonomous SQL agent."),
    HumanMessage(content="Generate an optimized PostgreSQL query for quarterly active users.")
]

response = llm.invoke(messages)
print(response.content)
```

---

## 3. CrewAI Agent Workflows

CrewAI agents can execute multi-step hierarchical workflows with memoized sub-tasks:

```python
from crewai import Agent, Task, Crew, LLM

# Configure LLM to route through Sunborn Gateway
sunborn_llm = LLM(
    model="gpt-4o",
    base_url="http://localhost:8080/v1",
    api_key="key-enterprise-fintech"
)

researcher = Agent(
    role="Principal Market Analyst",
    goal="Extract and synthesize competitive intelligence",
    backstory="You analyze enterprise SaaS architectures and compute cost structures.",
    llm=sunborn_llm,
    verbose=True
)

task = Task(
    description="Analyze latency benefits of semantic caching vs direct LLM calls.",
    expected_output="A structured 3-bullet summary with empirical metrics.",
    agent=researcher
)

crew = Crew(agents=[researcher], tasks=[task])
result = crew.kickoff()
print(result)
```

---

## 4. LlamaIndex RAG Pipelines

In LlamaIndex, connect the LLM and query engine:

```python
from llama_index.llms.openai import OpenAI
from llama_index.core import Settings

Settings.llm = OpenAI(
    model="gpt-4o",
    api_base="http://localhost:8080/v1",
    api_key="key-enterprise-fintech",
    temperature=0.1
)

# Any query engine or agent will now automatically reuse cached prompt embeddings
# and exact/semantic responses from Sunborn!
```

---

## 5. Agent Tool Execution Reuse with `@sunborn_tool`

For autonomous agents calling external tools (search engines, databases, calculation engines, APIs), the Sunborn Python SDK provides deterministic tool memoization via the Merkle DAG engine (`/v1/execution/plan`):

```python
from sdk.python.sunborn_agent import sunborn_tool

@sunborn_tool(name="query_customer_crm", version="1.0", deterministic=True)
def query_customer_crm(customer_id: str) -> dict:
    # This expensive API call executes ONLY ONCE.
    # Subsequent calls across any agent replica return in < 1ms from Sunborn!
    print(f"Executing real CRM lookup for customer {customer_id}...")
    return {
        "customer_id": customer_id,
        "tier": "enterprise",
        "arr": 120000,
        "status": "active"
    }

# 1st call: Miss -> executes real function and stores into Sunborn Execution Cache
res1 = query_customer_crm("cust_98721")

# 2nd call: Exact C4 Execution Hit -> returns in < 100 microseconds from Sunborn
res2 = query_customer_crm("cust_98721")
assert res1 == res2
```

---

## 6. Authentication & Tenant Header Mapping

When integrating agents, pass authentication via either:
1. `x-api-key`: `key-enterprise-fintech` (or `sunborn-admin-key`)
2. `Authorization: Bearer <key>`
3. `x-tenant-id`: Target enterprise tenant ID (must match key registry)
4. `x-application-id`: Target application ID
