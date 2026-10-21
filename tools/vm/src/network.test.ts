import {describe, expect, test} from 'bun:test'
import {diagnoseNetwork, parseProbe} from './network'

describe('diagnoseNetwork', () => {
  test('says nothing when DNS and the probe host answer', () => {
    expect(diagnoseNetwork(63933, {gateway: '192.168.2.1', dnsResolves: true, hostAnswers: true})).toBeUndefined()
  })

  test('names a missing default route', () => {
    expect(diagnoseNetwork(63933, {gateway: undefined, dnsResolves: false, hostAnswers: false})).toBe(
      'pr-qa-63933 has no network: no default route. Run vm restart 63933.'
    )
  })

  test('names a DNS failure', () => {
    expect(diagnoseNetwork(63933, {gateway: '192.168.2.1', dnsResolves: false, hostAnswers: false})).toBe(
      'pr-qa-63933 has no DNS: github.com does not resolve through gateway 192.168.2.1. Run vm restart 63933.'
    )
  })

  test('names a probe host that resolves but does not accept a connection', () => {
    expect(diagnoseNetwork(63933, {gateway: '192.168.2.1', dnsResolves: true, hostAnswers: false})).toBe(
      'pr-qa-63933 has no network: github.com:443 does not answer through gateway 192.168.2.1. Run vm restart 63933.'
    )
  })
})

describe('parseProbe', () => {
  test('reads a full probe output', () => {
    expect(parseProbe('gateway=192.168.2.1\ndns=ok\ntcp=fail\n')).toEqual({gateway: '192.168.2.1', dnsResolves: true, hostAnswers: false})
  })

  test('reads an empty gateway as no route', () => {
    expect(parseProbe('gateway=\ndns=fail\ntcp=fail\n')).toEqual({gateway: undefined, dnsResolves: false, hostAnswers: false})
  })

  test('returns undefined when the probe did not report', () => {
    expect(parseProbe('')).toBeUndefined()
    expect(parseProbe('gateway=192.168.2.1\nping=fail\ndns=ok\n')).toBeUndefined()
    expect(parseProbe('Error: no such VM')).toBeUndefined()
  })
})
