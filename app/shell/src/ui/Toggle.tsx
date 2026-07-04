// The pill toggle from the reference cards ("On -40", green when active). A controlled switch with a
// spring-animated thumb + track color crossfade, built on RN's Animated (no extra native dep). Named
// Toggle, not Switch, so it never collides with RN's `Switch` in Unistyles' component map.

import React, { useEffect, useRef } from 'react';
import { Animated, Pressable } from 'react-native';
import { StyleSheet, useUnistyles } from 'react-native-unistyles';

type Props = { value: boolean; onValueChange: (v: boolean) => void };

const TRACK_W = 52;
const THUMB = 24;
const PAD = 3;

export function Toggle({ value, onValueChange }: Props) {
  const { theme } = useUnistyles();
  const anim = useRef(new Animated.Value(value ? 1 : 0)).current;

  useEffect(() => {
    Animated.spring(anim, { toValue: value ? 1 : 0, useNativeDriver: false, speed: 16, bounciness: 6 }).start();
  }, [value, anim]);

  const trackColor = anim.interpolate({
    inputRange: [0, 1],
    outputRange: [theme.colors.surfaceHi, theme.colors.mint],
  });
  const translateX = anim.interpolate({ inputRange: [0, 1], outputRange: [0, TRACK_W - THUMB - PAD * 2] });

  return (
    <Pressable onPress={() => onValueChange(!value)} hitSlop={8}>
      <Animated.View style={[styles.track, { backgroundColor: trackColor }]}>
        <Animated.View style={[styles.thumb, { transform: [{ translateX }] }]} />
      </Animated.View>
    </Pressable>
  );
}

const styles = StyleSheet.create((t) => ({
  track: { width: TRACK_W, height: THUMB + PAD * 2, borderRadius: t.radius.pill, padding: PAD, justifyContent: 'center' },
  thumb: { width: THUMB, height: THUMB, borderRadius: t.radius.pill, backgroundColor: '#0C0E0D' },
}));
