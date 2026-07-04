// The channel directory screen: list + create + open a room. Rows are cards (the reference "device"
// list look), each with a hashed chip; the composer is a pinned input + mint create button.

import React, { useState } from 'react';
import { FlatList, Text, TextInput, TouchableOpacity, View } from 'react-native';
import { StyleSheet, useUnistyles } from 'react-native-unistyles';
import { Hash, Plus } from 'lucide-react-native';
import { useSession } from '../session/useSession';
import { useChannels } from './useChannels';
import { Card } from '../../ui/Card';

export function ChannelsScreen({ onOpen }: { onOpen: (channel: string) => void }): React.JSX.Element {
  const session = useSession();
  const { theme } = useUnistyles();
  const { channels, create, error } = useChannels(session?.workspace);
  const [draft, setDraft] = useState('');

  return (
    <View style={styles.screen}>
      {error !== '' && <Text style={styles.error}>{error}</Text>}
      <FlatList
        data={channels}
        keyExtractor={(c) => c.id}
        contentContainerStyle={styles.list}
        renderItem={({ item }) => (
          <TouchableOpacity activeOpacity={0.8} onPress={() => onOpen(item.id)}>
            <Card style={styles.row}>
              <View style={styles.chip}>
                <Hash size={18} color={theme.colors.mint} />
              </View>
              <Text style={styles.name}>{item.id}</Text>
            </Card>
          </TouchableOpacity>
        )}
        ListEmptyComponent={<Text style={styles.hint}>No channels yet — create one below.</Text>}
      />
      <View style={styles.composer}>
        <TextInput
          style={styles.input}
          value={draft}
          onChangeText={setDraft}
          placeholder="new-channel-id"
          placeholderTextColor={theme.colors.textFaint}
          autoCapitalize="none"
          autoCorrect={false}
        />
        <TouchableOpacity
          style={styles.button}
          activeOpacity={0.85}
          onPress={() => {
            if (draft.trim()) void create(draft.trim());
            setDraft('');
          }}>
          <Plus size={22} color={theme.colors.ink} />
        </TouchableOpacity>
      </View>
    </View>
  );
}

const styles = StyleSheet.create((t) => ({
  screen: { flex: 1, padding: t.space(4), backgroundColor: t.colors.ink },
  list: { gap: t.space(2.5), paddingBottom: t.space(3) },
  row: { flexDirection: 'row', alignItems: 'center', gap: t.space(3), paddingVertical: t.space(3.5) },
  chip: {
    width: 38,
    height: 38,
    borderRadius: t.radius.sm,
    backgroundColor: t.colors.surfaceHi,
    alignItems: 'center',
    justifyContent: 'center',
  },
  name: { color: t.colors.text, fontSize: 16, fontWeight: t.font.weightMed },
  hint: { color: t.colors.textDim, paddingVertical: t.space(4), textAlign: 'center' },
  error: { color: t.colors.danger, marginBottom: t.space(2) },
  composer: { flexDirection: 'row', gap: t.space(2.5), paddingTop: t.space(2) },
  input: {
    flex: 1,
    backgroundColor: t.colors.surface,
    color: t.colors.text,
    borderWidth: 1,
    borderColor: t.colors.line,
    borderRadius: t.radius.md,
    paddingHorizontal: t.space(4),
    paddingVertical: t.space(3.5),
    fontSize: 15,
  },
  button: {
    backgroundColor: t.colors.mint,
    borderRadius: t.radius.md,
    width: 52,
    alignItems: 'center',
    justifyContent: 'center',
  },
}));
