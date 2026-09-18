'use strict'

const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const os = require('node:os')
const path = require('node:path')

const docx2typst = require('..')

const repoRoot = path.resolve(__dirname, '../../..')
const headingsFixture = path.join(
  repoRoot,
  'tests/corpus/public/fixtures/powertools-headings.docx',
)

test('convertFile returns conversion and validation when output_path is provided', async () => {
  const outDir = await fs.mkdtemp(path.join(os.tmpdir(), 'docx2typst-node-file-'))
  const result = await docx2typst.convertFile(headingsFixture, {
    output_mode: 'bundle',
    output_path: outDir,
  })

  assert.ok(result.conversion)
  assert.ok(result.validation)
  assert.equal(result.validation.compile_ok, true)
  await fs.rm(outDir, { recursive: true, force: true })
})

test('convertBuffer mirrors convertFile validation behavior and honors reference_path', async () => {
  const fixtureBuffer = await fs.readFile(headingsFixture)
  const referencePdf = path.join(
    os.tmpdir(),
    `docx2typst-node-missing-${Date.now()}.pdf`,
  )

  const compareResult = await docx2typst.convertBuffer(fixtureBuffer, {
    reference: referencePdf,
  })

  assert.ok(compareResult.validation)
  assert.equal(compareResult.validation.reference ?? null, null)
  assert.ok(
    compareResult.validation.diagnostics.some(
      diagnostic => diagnostic.code === 'REFERENCE_PDF_LOAD_ERROR',
    ),
  )
})
