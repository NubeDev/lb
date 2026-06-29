import { Footer, Layout, Navbar } from 'nextra-theme-docs'
import { Head } from 'nextra/components'
import { getPageMap } from 'nextra/page-map'
import 'nextra-theme-docs/style.css'
import './globals.css'

// Source repo link. Set DOCS_REPO_URL in the environment to point the navbar /
// "Edit this page" links at the repository.
const repoUrl = process.env.DOCS_REPO_URL || 'https://github.com/NubeIO/lb'

export const metadata = {
  title: {
    default: 'Lazybones Docs',
    template: '%s – Lazybones Docs',
  },
  description:
    'Lazybones — a reusable edge/cloud platform for extension-driven, local-first workspace software.',
}

const navbar = <Navbar logo={<b>Lazybones</b>} projectLink={repoUrl} />

const footer = <Footer>{new Date().getFullYear()} © Lazybones</Footer>

export default async function RootLayout({ children }) {
  const pageMap = await getPageMap()
  return (
    <html lang="en" dir="ltr" suppressHydrationWarning>
      <Head />
      {/* Browser extensions inject attributes onto <body> before React hydrates,
          tripping a hydration mismatch warning. Suppress it here like on <html>. */}
      <body suppressHydrationWarning>
        <Layout
          navbar={navbar}
          footer={footer}
          pageMap={pageMap}
          docsRepositoryBase={`${repoUrl}/tree/main/doc-site`}
          sidebar={{ toggleButton: true, defaultMenuCollapseLevel: 1 }}
        >
          {children}
        </Layout>
      </body>
    </html>
  )
}
