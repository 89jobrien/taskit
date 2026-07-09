client<llm> Anthropic_Sonnet {
    provider anthropic
    retry_policy Exponential
    options {
        model "claude-sonnet-4-6"
        api_key env.ANTHROPIC_API_KEY
    }
}

client<llm> OpenAI_GPT4o {
    provider openai
    retry_policy Exponential
    options {
        model "gpt-4o"
        api_key env.OPENAI_API_KEY
    }
}

client<llm> ConflictResolver {
    provider fallback
    options {
        strategy [Anthropic_Sonnet, OpenAI_GPT4o]
    }
}

retry_policy Exponential {
    max_retries 2
    strategy {
        type exponential_backoff
        delay_ms 300
        multiplier 1.5
        max_delay_ms 8000
    }
}
