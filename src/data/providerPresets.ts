import type { AppType } from "@/types";

/**
 * 供应商预设:添加供应商时按分组展示,点选后预填名称与接口地址。
 * 端点值参考 CC Switch 预设库(MIT)与各家官方文档,均为事实性字段;
 * apiKeyUrl 一律使用官方控制台地址,不带推广参数。
 */

export type PresetCategory =
  | "official"
  | "cn_official"
  | "coding_plan"
  | "aggregator"
  | "third_party";

export interface ProviderPreset {
  name: string;
  baseUrl: string;
  category: PresetCategory;
  /** 品牌图标 key,对应 ProviderIcon 的 BRAND 表 */
  icon?: string;
  websiteUrl?: string;
  /** 获取 API Key 的控制台地址 */
  apiKeyUrl?: string;
  /**
   * 合作伙伴/赞助位(预留):UI 上会显示「赞助」角标。
   * 收录规则见 CONTRIBUTING.md;apiKeyUrl 必须保持官方地址,
   * 推广参数只允许出现在 websiteUrl,且需明示。
   */
  partner?: boolean;
  /** 内置模型列表(仅信息展示 + 创建时预填,可在编辑时修改) */
  models?: string[];
}

/** OpenAI 兼容通用端点,opencode / openclaw 共用 */
const openaiCompatible: ProviderPreset[] = [
  {
    name: "CoderPlan",
    icon: "coderplan",
    baseUrl: "https://api.coderplan.ai/v1",
    category: "aggregator",
    websiteUrl: "https://coderplan.ai",
    apiKeyUrl: "https://coderplan.ai/dashboard/keys",
    models: ["gpt-5.5", "gpt-5.4-mini", "deepseek-v4-pro", "minimax-m2.5"],
  },
  {
    name: "TheRouter",
    baseUrl: "https://therouter.ai/v1",
    category: "aggregator",
    websiteUrl: "https://therouter.ai",
    apiKeyUrl: "https://therouter.ai/user/settings",
  },
  {
    name: "DeepSeek",
    icon: "deepseek",
    baseUrl: "https://api.deepseek.com",
    category: "cn_official",
    websiteUrl: "https://platform.deepseek.com",
    apiKeyUrl: "https://platform.deepseek.com/api_keys",
  },
  {
    name: "Zhipu GLM",
    icon: "zhipu",
    baseUrl: "https://open.bigmodel.cn/api/paas/v4",
    category: "cn_official",
    websiteUrl: "https://open.bigmodel.cn",
    apiKeyUrl: "https://open.bigmodel.cn/usercenter/apikeys",
  },
  {
    name: "Kimi (Moonshot)",
    icon: "kimi",
    baseUrl: "https://api.moonshot.cn/v1",
    category: "cn_official",
    websiteUrl: "https://platform.moonshot.cn",
    apiKeyUrl: "https://platform.moonshot.cn/console/api-keys",
  },
    {
      name: "SiliconFlow",
      icon: "siliconflow",
      baseUrl: "https://api.siliconflow.cn/v1",
      category: "cn_official",
      websiteUrl: "https://siliconflow.cn",
      apiKeyUrl: "https://cloud.siliconflow.cn/account/ak",
    },
  ];

/** Anthropic 兼容端点预设(Claude / iFlow / Droid 分组共用) */
const anthropicCompatible: ProviderPreset[] = [
  {
    name: "CoderPlan",
    icon: "coderplan",
    baseUrl: "https://api.coderplan.ai",
    category: "aggregator",
    websiteUrl: "https://coderplan.ai",
    apiKeyUrl: "https://coderplan.ai/dashboard/keys",
    models: ["claude-sonnet-5", "gpt-5.5", "deepseek-v4-pro", "minimax-m2.5"],
  },
  {
    name: "TheRouter",
    baseUrl: "https://therouter.ai",
    category: "aggregator",
    websiteUrl: "https://therouter.ai",
    apiKeyUrl: "https://therouter.ai/user/settings",
  },
  {
    name: "Claude 官方",
    icon: "claude",
    baseUrl: "https://api.anthropic.com",
    category: "official",
    websiteUrl: "https://www.anthropic.com/claude-code",
    apiKeyUrl: "https://console.anthropic.com/settings/keys",
  },
  {
    name: "Zhipu GLM",
    icon: "zhipu",
    baseUrl: "https://open.bigmodel.cn/api/anthropic",
    category: "cn_official",
    websiteUrl: "https://open.bigmodel.cn",
    apiKeyUrl: "https://open.bigmodel.cn/usercenter/apikeys",
  },
  {
    name: "Zhipu GLM 国际版",
    icon: "zhipu",
    baseUrl: "https://api.z.ai/api/anthropic",
    category: "cn_official",
    websiteUrl: "https://z.ai",
  },
  {
    name: "Kimi",
    icon: "kimi",
    baseUrl: "https://api.moonshot.cn/anthropic",
    category: "cn_official",
    websiteUrl: "https://platform.moonshot.cn",
    apiKeyUrl: "https://platform.moonshot.cn/console/api-keys",
  },
  {
    name: "Zhipu GLM Coding Plan",
    icon: "zhipu",
    baseUrl: "https://open.bigmodel.cn/api/coding/paas-v4",
    category: "coding_plan",
    websiteUrl: "https://bigmodel.cn/claude-code",
    apiKeyUrl: "https://bigmodel.cn/usercenter/apikeys",
  },
  {
    name: "Kimi For Coding",
    icon: "kimi",
    baseUrl: "https://api.kimi.com/coding/",
    category: "coding_plan",
    websiteUrl: "https://www.kimi.com/code/docs/",
  },
  {
    name: "DeepSeek",
    icon: "deepseek",
    baseUrl: "https://api.deepseek.com/anthropic",
    category: "cn_official",
    websiteUrl: "https://platform.deepseek.com",
    apiKeyUrl: "https://platform.deepseek.com/api_keys",
  },
  {
    name: "豆包 Coding Plan",
    icon: "doubao",
    baseUrl: "https://ark.cn-beijing.volces.com/api/coding",
    category: "coding_plan",
    websiteUrl: "https://www.volcengine.com/product/doubao",
    apiKeyUrl: "https://console.volcengine.com/ark",
  },
  {
    name: "阿里云百灵 Coding",
    icon: "qwen",
    baseUrl: "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    category: "coding_plan",
    websiteUrl: "https://bailian.console.aliyun.com",
  },
  {
    name: "百度千帆 Coding Plan",
    baseUrl: "https://qianfan.baidubce.com/anthropic/coding",
    category: "coding_plan",
    websiteUrl: "https://cloud.baidu.com/product/qianfan_modelbuilder",
  },
  {
    name: "MiniMax",
    icon: "minimax",
    baseUrl: "https://api.minimaxi.com/anthropic",
    category: "coding_plan",
    websiteUrl: "https://platform.minimaxi.com",
    apiKeyUrl: "https://platform.minimaxi.com/subscribe/coding-plan",
  },
  {
    name: "阶跃星辰 StepFun",
    icon: "stepfun",
    baseUrl: "https://api.stepfun.com/step_plan",
    category: "coding_plan",
    websiteUrl: "https://platform.stepfun.com/step-plan",
    apiKeyUrl: "https://platform.stepfun.com/interface-key",
  },
  {
    name: "魔搭 ModelScope",
    icon: "modelscope",
    baseUrl: "https://api-inference.modelscope.cn",
    category: "cn_official",
    websiteUrl: "https://modelscope.cn",
    apiKeyUrl: "https://modelscope.cn/my/mykeys",
  },
  {
    name: "SiliconFlow",
    icon: "siliconflow",
    baseUrl: "https://api.siliconflow.cn",
    category: "cn_official",
    websiteUrl: "https://siliconflow.cn",
    apiKeyUrl: "https://cloud.siliconflow.cn/account/ak",
  },
];

export const providerPresets: Record<AppType, ProviderPreset[]> = {
  // Claude 分组:Anthropic 兼容端点
  claude: anthropicCompatible,
  // Codex 分组:OpenAI 兼容 / Responses 端点
  codex: [
    {
      name: "OpenAI 官方",
      icon: "openai",
      baseUrl: "https://api.openai.com/v1",
      category: "official",
      websiteUrl: "https://chatgpt.com/codex",
      apiKeyUrl: "https://platform.openai.com/api-keys",
    },
    ...openaiCompatible,
  ],
  // Gemini 分组:Gemini 原生端点
  gemini: [
    {
      name: "Google 官方",
      icon: "gemini",
      baseUrl: "https://generativelanguage.googleapis.com",
      category: "official",
      websiteUrl: "https://ai.google.dev",
      apiKeyUrl: "https://aistudio.google.com/apikey",
    },
  ],
  opencode: openaiCompatible,
  openclaw: openaiCompatible,
  // Qwen Code CLI:OpenAI 兼容端点
  qwen: [
    {
      name: "阿里云百炼(兼容模式)",
      icon: "qwen",
      baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
      category: "cn_official",
      websiteUrl: "https://bailian.console.aliyun.com",
      apiKeyUrl: "https://bailian.console.aliyun.com/?apiKey=1",
    },
    ...openaiCompatible,
  ],
  // iFlow CLI:Anthropic 兼容端点
  iflow: anthropicCompatible,
  // Crush CLI:OpenAI 兼容端点
  crush: openaiCompatible,
  // Droid CLI:Anthropic 兼容端点
  droid: anthropicCompatible,
};

/** 类别展示顺序(custom 固定在最后,由对话框自行追加) */
export const presetCategoryOrder: PresetCategory[] = [
  "official",
  "cn_official",
  "coding_plan",
  "aggregator",
  "third_party",
];
