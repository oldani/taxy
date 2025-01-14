<template>
    <v-container>
        <v-row>
            <v-col cols="12" sm="12">
                <v-select :label="$t('acme.challenge')" :items="challenges" v-model="acmeChallange" variant="outlined"
                    density="compact"></v-select>
            </v-col>
            <v-col cols="12" sm="12">
                <v-text-field @update:modelValue="update" type="email" :label="$t('acme.email')" variant="outlined"
                    v-model="email" density="compact" :rules="emailRules" persistent-hint></v-text-field>
            </v-col>
            <v-col cols="12" sm="12">
                <v-text-field @update:modelValue="update" autocapitalize="off" :label="$t('acme.domain')" variant="outlined"
                    v-model="domain" density="compact" :rules="domainNameRules" persistent-hint></v-text-field>
            </v-col>
        </v-row>
    </v-container>
</template>

<script setup>
import { ref, defineProps, defineEmits, computed } from 'vue'
import { isValidHostname } from '@/utils/validators'
import { useI18n } from 'vue-i18n'

const { t } = useI18n({ useScope: 'global' })

const props = defineProps({
    staging: {
        type: Boolean,
        default: false,
    },
    modelValue: {
        type: Object,
        default: () => ({}),
    },
});

const emit = defineEmits(['update:modelValue']);

const update = (value) => {
    emit('update:modelValue', {
        challenge_type: acmeChallange.value,
        renewal_days: 60,
        identifiers: [
            value
        ],
        provider: props.staging ? "Let's Encrypt (Staging)" : "Let's Encrypt",
        server_url: props.staging ? "https://acme-staging-v02.api.letsencrypt.org/directory" : "https://acme-v02.api.letsencrypt.org/directory",
        contacts: [`mailto:${email.value}`],
        is_trusted: !props.staging,
    })
}

const acmeChallange = ref('http-01');
const domain = ref('');
const email = ref('');

const challenges = [
    { title: 'HTTP', value: 'http-01' }
]

const domainNameRules = [
    value => {
        if (isValidHostname(value)) return true
        return t('acme.add_acme.rule.hostname_required')
    },
]

const emailRules = [
    value => {
        if (/\S+@\S+\.\S+/.test(value)) return true
        return t('acme.add_acme.rule.email_required')
    },
]
</script>