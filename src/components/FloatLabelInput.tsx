import React, { useState } from 'react'
import { Input, InputProps } from 'antd'
import styles from './FloatLabelInput.module.css'
import { cn } from '../utils.ts'

interface FloatLabelInputProps extends Omit<InputProps, 'prefix' | 'placeholder'> {
  label: string
}

export function FloatLabelInput({
  label,
  value,
  size = 'middle',
  onFocus,
  onBlur,
  className,
  ...restProps
}: FloatLabelInputProps) {
  const [focused, setFocused] = useState<boolean>(false)

  const floating = (value !== undefined && value !== null && String(value).length > 0) || focused

  function getTranslateY() {
    switch (size) {
      case 'small':
        return 'translate-y-[-90%]'
      case 'middle':
        return 'translate-y-[-130%]'
      case 'large':
        return 'translate-y-[-165%]'
    }
  }

  const handleFocus = (e: React.FocusEvent<HTMLInputElement>) => {
    setFocused(true)
    onFocus?.(e)
  }

  const handleBlur = (e: React.FocusEvent<HTMLInputElement>) => {
    setFocused(false)
    onBlur?.(e)
  }

  return (
    <Input
      {...restProps}
      className={cn(styles.floatLabelInput, className)}
      prefix={
        <span
          className={cn(
            styles.floatLabel,
            'bg-white transition-all duration-200 ease-in-out',
            floating && `text-0.75rem px-0.5 ${getTranslateY()}`,
          )}>
          {label}
        </span>
      }
      value={value}
      size={size}
      onFocus={handleFocus}
      onBlur={handleBlur}
      placeholder=""
    />
  )
}
