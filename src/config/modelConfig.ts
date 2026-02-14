import { Gemini, Claude, OpenAI } from '@lobehub/icons';

/**
 * 模型配置接口
 */
export interface ModelConfig {
    /** 模型完整显示名称 (用于详情) */
    label: string;
    /** 模型简短标签 (用于列表/卡片) */
    shortLabel: string;
    /** 保护模型的键名 */
    protectedKey: string;
    /** 模型图标组件 */
    Icon: React.ComponentType<{ size?: number; className?: string }>;
}

/**
 * 模型配置映射
 * 键为模型 ID，值为模型配置
 * Copilot-supported models only
 */
export const MODEL_CONFIG: Record<string, ModelConfig> = {
    // GPT 系列
    'gpt-4o': {
        label: 'GPT-4o',
        shortLabel: 'GPT-4o',
        protectedKey: 'gpt-4o',
        Icon: OpenAI,
    },
    'gpt-4o-mini': {
        label: 'GPT-4o Mini',
        shortLabel: 'GPT-4o Mini',
        protectedKey: 'gpt-4o-mini',
        Icon: OpenAI,
    },
    'gpt-4.1': {
        label: 'GPT-4.1',
        shortLabel: 'GPT-4.1',
        protectedKey: 'gpt-4.1',
        Icon: OpenAI,
    },
    'gpt-4.1-mini': {
        label: 'GPT-4.1 Mini',
        shortLabel: 'GPT-4.1 Mini',
        protectedKey: 'gpt-4.1-mini',
        Icon: OpenAI,
    },
    'gpt-4.1-nano': {
        label: 'GPT-4.1 Nano',
        shortLabel: 'GPT-4.1 Nano',
        protectedKey: 'gpt-4.1-nano',
        Icon: OpenAI,
    },

    // O 系列
    'o1': {
        label: 'O1',
        shortLabel: 'O1',
        protectedKey: 'o1',
        Icon: OpenAI,
    },
    'o1-mini': {
        label: 'O1 Mini',
        shortLabel: 'O1 Mini',
        protectedKey: 'o1-mini',
        Icon: OpenAI,
    },
    'o3': {
        label: 'O3',
        shortLabel: 'O3',
        protectedKey: 'o3',
        Icon: OpenAI,
    },
    'o3-mini': {
        label: 'O3 Mini',
        shortLabel: 'O3 Mini',
        protectedKey: 'o3-mini',
        Icon: OpenAI,
    },
    'o4-mini': {
        label: 'O4 Mini',
        shortLabel: 'O4 Mini',
        protectedKey: 'o4-mini',
        Icon: OpenAI,
    },

    // Claude 系列
    'claude-3.5-sonnet': {
        label: 'Claude 3.5 Sonnet',
        shortLabel: 'Claude 3.5',
        protectedKey: 'claude',
        Icon: Claude.Color,
    },
    'claude-sonnet-4': {
        label: 'Claude Sonnet 4',
        shortLabel: 'Claude 4',
        protectedKey: 'claude',
        Icon: Claude.Color,
    },
    'claude-sonnet-4-5': {
        label: 'Claude Sonnet 4.5',
        shortLabel: 'Claude 4.5',
        protectedKey: 'claude',
        Icon: Claude.Color,
    },

    // Gemini 系列
    'gemini-2.0-flash': {
        label: 'Gemini 2.0 Flash',
        shortLabel: 'G2.0 Flash',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
    },
    'gemini-2.5-pro': {
        label: 'Gemini 2.5 Pro',
        shortLabel: 'G2.5 Pro',
        protectedKey: 'gemini-pro',
        Icon: Gemini.Color,
    },
};

/**
 * 获取所有模型 ID 列表
 */
export const getAllModelIds = (): string[] => Object.keys(MODEL_CONFIG);

/**
 * 根据模型 ID 获取配置
 */
export const getModelConfig = (modelId: string): ModelConfig | undefined => {
    return MODEL_CONFIG[modelId.toLowerCase()];
};

/**
 * 模型排序权重配置
 * 数字越小，优先级越高
 */
const MODEL_SORT_WEIGHTS = {
    // 系列权重 (第一优先级)
    series: {
        'gpt': 100,
        'o': 200,
        'claude': 300,
        'gemini': 400,
    },
    // 性能级别权重 (第二优先级)
    tier: {
        'nano': 30,
        'mini': 20,
        'standard': 10,
        'pro': 5,
        'sonnet': 10,
        'flash': 20,
    },
};

/**
 * 获取模型的排序权重
 */
function getModelSortWeight(modelId: string): number {
    const id = modelId.toLowerCase();
    let weight = 0;

    // 1. 系列权重 (x1000)
    if (id.startsWith('gpt')) {
        weight += MODEL_SORT_WEIGHTS.series['gpt'] * 1000;
    } else if (id.startsWith('o') && /^o\d/.test(id)) {
        weight += MODEL_SORT_WEIGHTS.series['o'] * 1000;
    } else if (id.startsWith('claude')) {
        weight += MODEL_SORT_WEIGHTS.series['claude'] * 1000;
    } else if (id.startsWith('gemini')) {
        weight += MODEL_SORT_WEIGHTS.series['gemini'] * 1000;
    }

    // 2. 性能级别权重 (x100)
    if (id.includes('nano')) {
        weight += MODEL_SORT_WEIGHTS.tier['nano'] * 100;
    } else if (id.includes('mini')) {
        weight += MODEL_SORT_WEIGHTS.tier['mini'] * 100;
    } else if (id.includes('pro')) {
        weight += MODEL_SORT_WEIGHTS.tier['pro'] * 100;
    } else if (id.includes('sonnet')) {
        weight += MODEL_SORT_WEIGHTS.tier['sonnet'] * 100;
    } else if (id.includes('flash')) {
        weight += MODEL_SORT_WEIGHTS.tier['flash'] * 100;
    } else {
        weight += MODEL_SORT_WEIGHTS.tier['standard'] * 100;
    }

    return weight;
}

/**
 * 对模型列表进行排序
 * @param models 模型列表
 * @returns 排序后的模型列表
 */
export function sortModels<T extends { id: string }>(models: T[]): T[] {
    return [...models].sort((a, b) => {
        const weightA = getModelSortWeight(a.id);
        const weightB = getModelSortWeight(b.id);

        // 按权重升序排序
        if (weightA !== weightB) {
            return weightA - weightB;
        }

        // 权重相同时，按字母顺序排序
        return a.id.localeCompare(b.id);
    });
}
