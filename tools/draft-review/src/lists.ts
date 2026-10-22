export const unique = <Value>(values: readonly Value[]): Value[] => [...new Set(values)]

export const duplicates = <Value>(values: readonly Value[]): Value[] => unique(values.filter((value, index) => values.indexOf(value) !== index))

export const isDefined = <Value>(value: Value | undefined): value is Value => value !== undefined
