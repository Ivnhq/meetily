'use client';

import { Bot, Link2, RotateCcw } from 'lucide-react';
import { useConfig } from '@/contexts/ConfigContext';
import { isLiveNotesProviderSupported, ModelConfig } from '@/services/configService';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';

const LIVE_NOTES_PROVIDER_LABELS: Record<ModelConfig['provider'], string> = {
  ollama: 'Ollama',
  'builtin-ai': 'Built-in AI',
  'custom-openai': 'Custom Server',
  claude: 'Claude',
  groq: 'Groq',
  openai: 'OpenAI',
  openrouter: 'OpenRouter',
};

function createOverrideConfig(
  provider: ModelConfig['provider'],
  modelConfig: ModelConfig,
  modelOptions: Record<ModelConfig['provider'], string[]>
): ModelConfig {
  const availableModels = modelOptions[provider] ?? [];
  const fallbackModel =
    provider === modelConfig.provider && isLiveNotesProviderSupported(modelConfig.provider)
      ? modelConfig.model
      : availableModels[0] ?? '';

  return {
    ...modelConfig,
    provider,
    model: fallbackModel,
    customOpenAIEndpoint: provider === 'custom-openai' ? modelConfig.customOpenAIEndpoint ?? null : null,
    customOpenAIModel: provider === 'custom-openai' ? modelConfig.customOpenAIModel ?? fallbackModel : null,
    customOpenAIApiKey: provider === 'custom-openai' ? modelConfig.customOpenAIApiKey ?? null : null,
    maxTokens: provider === 'custom-openai' ? modelConfig.maxTokens ?? null : null,
    temperature: provider === 'custom-openai' ? modelConfig.temperature ?? null : null,
    topP: provider === 'custom-openai' ? modelConfig.topP ?? null : null,
  };
}

export function LiveNotesModelSettings() {
  const {
    modelConfig,
    modelOptions,
    liveNotesModelConfig,
    effectiveLiveNotesModelConfig,
    setLiveNotesModelConfig,
  } = useConfig();

  const usesSummaryModel = liveNotesModelConfig === null;
  const selectedConfig = liveNotesModelConfig ?? effectiveLiveNotesModelConfig;
  const selectedProvider = usesSummaryModel ? 'summary' : selectedConfig.provider;
  const availableModels = modelOptions[selectedConfig.provider] ?? [];
  const summarySupported = isLiveNotesProviderSupported(modelConfig.provider);

  const updateOverride = (patch: Partial<ModelConfig>) => {
    setLiveNotesModelConfig((current) => ({
      ...(current ?? createOverrideConfig('ollama', modelConfig, modelOptions)),
      ...patch,
    }));
  };

  return (
    <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm">
      <div className="mb-4 flex items-start justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <Bot className="h-4 w-4 text-blue-600" />
            <h3 className="text-lg font-semibold text-gray-900">Live Notes Model</h3>
          </div>
          <p className="mt-1 text-sm text-gray-600">
            Used for in-meeting notes, recaps, actions, and decisions.
          </p>
        </div>

        {!usesSummaryModel && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => setLiveNotesModelConfig(null)}
          >
            <RotateCcw className="h-4 w-4" />
            Reset
          </Button>
        )}
      </div>

      <div className="grid gap-4 md:grid-cols-[220px_1fr]">
        <div>
          <Label>Provider</Label>
          <Select
            value={selectedProvider}
            onValueChange={(value) => {
              if (value === 'summary') {
                setLiveNotesModelConfig(null);
                return;
              }

              setLiveNotesModelConfig(createOverrideConfig(
                value as ModelConfig['provider'],
                modelConfig,
                modelOptions
              ));
            }}
          >
            <SelectTrigger className="mt-1">
              <SelectValue placeholder="Select provider" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="summary">
                Summary Model{summarySupported ? '' : ' (unsupported)'}
              </SelectItem>
              <SelectItem value="ollama">Ollama</SelectItem>
              <SelectItem value="builtin-ai">Built-in AI</SelectItem>
              <SelectItem value="custom-openai">Custom Server</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div>
          <Label>Model</Label>
          {selectedConfig.provider === 'ollama' && availableModels.length > 0 ? (
            <Select
              value={selectedConfig.model}
              disabled={usesSummaryModel}
              onValueChange={(model) => updateOverride({ model })}
            >
              <SelectTrigger className="mt-1">
                <SelectValue placeholder="Select model" />
              </SelectTrigger>
              <SelectContent>
                {availableModels.map((model) => (
                  <SelectItem key={model} value={model}>{model}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          ) : (
            <Input
              className="mt-1"
              value={selectedConfig.model}
              disabled={usesSummaryModel}
              onChange={(event) => updateOverride({
                model: event.target.value,
                customOpenAIModel: selectedConfig.provider === 'custom-openai'
                  ? event.target.value
                  : selectedConfig.customOpenAIModel,
              })}
              placeholder="Model name"
            />
          )}
        </div>
      </div>

      {selectedConfig.provider === 'custom-openai' && !usesSummaryModel && (
        <div className="mt-4 grid gap-4 md:grid-cols-[1fr_220px]">
          <div>
            <Label>Endpoint</Label>
            <div className="relative mt-1">
              <Link2 className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-400" />
              <Input
                className="pl-9"
                value={selectedConfig.customOpenAIEndpoint ?? ''}
                onChange={(event) => updateOverride({ customOpenAIEndpoint: event.target.value })}
                placeholder="http://localhost:8000/v1"
              />
            </div>
          </div>
          <div>
            <Label>API Key</Label>
            <Input
              className="mt-1"
              type="password"
              value={selectedConfig.customOpenAIApiKey ?? ''}
              onChange={(event) => updateOverride({ customOpenAIApiKey: event.target.value || null })}
              placeholder="Optional"
            />
          </div>
        </div>
      )}

      <div className="mt-4 rounded-md border border-blue-100 bg-blue-50 px-3 py-2 text-sm text-blue-800">
        Active: {LIVE_NOTES_PROVIDER_LABELS[selectedConfig.provider]} / {selectedConfig.model || 'No model selected'}
      </div>
    </div>
  );
}
