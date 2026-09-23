import { ChannelRoom } from '@/components/ChannelRoom'

export default async function Page({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params
  return <ChannelRoom slug={slug} />
}
