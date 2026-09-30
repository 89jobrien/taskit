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

// Local last-resort client. Needs no credential, so it is always
// constructible; it is only reached when a remote provider's call fails.
client<llm> Ollama_Llama {
    provider ollama
    retry_policy Exponential
    options {
        model "llama3.2"
    }
}

// Resolution order: remote providers first, local Ollama last. A provider
// whose credential is present but rejected (401/5xx) advances the chain, so
// an expired key degrades to the local daemon rather than failing the flow.
//
// A credential that is *absent* is different: BAML cannot construct that
// client at all and aborts the whole chain (boundaryml/baml#2055), so
// `src/flow_resolver.rs` pins the call to `Ollama_Llama` when neither
// credential is configured.
client<llm> ConflictResolver {
    provider fallback
    options {
        strategy [Anthropic_Sonnet, OpenAI_GPT4o, Ollama_Llama]
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
