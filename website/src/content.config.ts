import { defineCollection } from 'astro:content';
import { z } from 'astro/zod';
import { glob } from 'astro/loaders';

const docsSchema = z.object({
  title: z.string(),
  description: z.string().optional(),
  badge: z.string().optional(),
  sidebarId: z.string().optional(),
  sidebarGroup: z.string().optional(),
  order: z.number().optional().default(99),
  published: z.boolean().optional().default(true),
});

const docs = defineCollection({
  loader: glob({ pattern: '**/*.mdx', base: './src/content/docs' }),
  schema: docsSchema,
});

// Russian translations of docs entries (matched by slug). When a ru file
// exists for a slug, it is rendered directly at /ru/docs/<slug> and the
// i18n post-generator skips stub generation for that path.
const docsRu = defineCollection({
  loader: glob({ pattern: '**/*.mdx', base: './src/content/docs-ru' }),
  schema: docsSchema,
});

export const collections = { docs, docsRu };