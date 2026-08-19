import { defineCollection, z } from 'astro:content';
import { glob } from 'astro/loaders';

const docs = defineCollection({
  loader: glob({ pattern: '**/*.mdx', base: './src/content/docs' }),
  schema: z.object({
    title: z.string(),
    description: z.string().optional(),
    badge: z.string().optional(),
    sidebarId: z.string().optional(),
    sidebarGroup: z.string().optional(),
    order: z.number().optional().default(99),
    published: z.boolean().optional().default(true),
  }),
});

export const collections = { docs };